package main

import (
	"crypto"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"errors"
	"fmt"
	"math/big"
	"os"
	"path/filepath"
	"slices"
	"sync"
	"syscall"
	"time"
)

// permittedDomains are the names the CA may sign for. A dNSName constraint also matches the
// subdomains, so this covers every host in githubHosts.
var permittedDomains = []string{"github.com"}

// authority is the CA that signs the certificates ghshim presents for the GitHub hosts. Only
// the processes that `ghshim exec` starts trust it.
type authority struct {
	cert    *x509.Certificate
	key     crypto.Signer
	certPEM []byte

	mu     sync.Mutex
	leaves map[string]*tls.Certificate
}

// loadOrCreateAuthority loads the CA, creating it when it is missing, about to expire, or not
// constrained to permittedDomains. A CA that an older ghshim left in an environment snapshot
// is replaced that way.
func loadOrCreateAuthority(p paths) (*authority, error) {
	if err := os.MkdirAll(p.state, 0o700); err != nil {
		return nil, err
	}
	lock, err := lockFile(filepath.Join(p.state, "ca.lock"), syscall.LOCK_EX)
	if err != nil {
		return nil, err
	}
	defer lock.Close()

	certPEM, certErr := os.ReadFile(p.caCert())
	keyPEM, keyErr := os.ReadFile(p.caKey())
	switch {
	case certErr == nil && keyErr == nil:
		if ca, err := parseAuthority(certPEM, keyPEM); err == nil && usableAuthority(ca.cert) {
			return ca, nil
		}
	case certErr != nil && !errors.Is(certErr, os.ErrNotExist):
		return nil, certErr
	case keyErr != nil && !errors.Is(keyErr, os.ErrNotExist):
		return nil, keyErr
	}
	if certPEM, keyPEM, err = newAuthorityPEM(); err != nil {
		return nil, err
	}
	// The key goes first, so a certificate on disk always has its key.
	if err := writeFileAtomic(p.caKey(), keyPEM, 0o600); err != nil {
		return nil, err
	}
	if err := writeFileAtomic(p.caCert(), certPEM, 0o644); err != nil {
		return nil, err
	}
	return parseAuthority(certPEM, keyPEM)
}

func usableAuthority(cert *x509.Certificate) bool {
	return cert.PermittedDNSDomainsCritical &&
		slices.Equal(cert.PermittedDNSDomains, permittedDomains) &&
		time.Until(cert.NotAfter) > 30*24*time.Hour
}

func newAuthorityPEM() (certPEM, keyPEM []byte, err error) {
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, nil, err
	}
	serial, err := randomSerial()
	if err != nil {
		return nil, nil, err
	}
	now := time.Now()
	template := &x509.Certificate{
		SerialNumber:          serial,
		Subject:               pkix.Name{CommonName: "ghshim local CA"},
		NotBefore:             now.Add(-time.Hour),
		NotAfter:              now.AddDate(1, 0, 0),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		BasicConstraintsValid: true,
		IsCA:                  true,
		MaxPathLenZero:        true,
		// The programs that `ghshim exec` starts trust the CA, so a leaked key must not let
		// anyone impersonate other hosts to them.
		PermittedDNSDomainsCritical: true,
		PermittedDNSDomains:         permittedDomains,
	}
	der, err := x509.CreateCertificate(rand.Reader, template, template, &key.PublicKey, key)
	if err != nil {
		return nil, nil, err
	}
	keyDER, err := x509.MarshalPKCS8PrivateKey(key)
	if err != nil {
		return nil, nil, err
	}
	certPEM = pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der})
	keyPEM = pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: keyDER})
	return certPEM, keyPEM, nil
}

func parseAuthority(certPEM, keyPEM []byte) (*authority, error) {
	certBlock, _ := pem.Decode(certPEM)
	keyBlock, _ := pem.Decode(keyPEM)
	if certBlock == nil || keyBlock == nil {
		return nil, errors.New("CA certificate or key is not PEM")
	}
	cert, err := x509.ParseCertificate(certBlock.Bytes)
	if err != nil {
		return nil, err
	}
	key, err := x509.ParsePKCS8PrivateKey(keyBlock.Bytes)
	if err != nil {
		return nil, err
	}
	signer, ok := key.(crypto.Signer)
	if !ok {
		return nil, fmt.Errorf("CA key of type %T cannot sign", key)
	}
	return &authority{cert: cert, key: signer, certPEM: certPEM, leaves: map[string]*tls.Certificate{}}, nil
}

// leaf returns a certificate for host signed by the CA, creating it on first use.
func (a *authority) leaf(host string) (*tls.Certificate, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if cert, ok := a.leaves[host]; ok && time.Until(cert.Leaf.NotAfter) > time.Hour {
		return cert, nil
	}
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, err
	}
	serial, err := randomSerial()
	if err != nil {
		return nil, err
	}
	now := time.Now()
	template := &x509.Certificate{
		SerialNumber: serial,
		Subject:      pkix.Name{CommonName: host},
		DNSNames:     []string{host},
		NotBefore:    now.Add(-time.Hour),
		NotAfter:     now.AddDate(0, 0, 30),
		KeyUsage:     x509.KeyUsageDigitalSignature,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	der, err := x509.CreateCertificate(rand.Reader, template, a.cert, &key.PublicKey, a.key)
	if err != nil {
		return nil, err
	}
	leaf, err := x509.ParseCertificate(der)
	if err != nil {
		return nil, err
	}
	cert := &tls.Certificate{Certificate: [][]byte{der}, PrivateKey: key, Leaf: leaf}
	a.leaves[host] = cert
	return cert, nil
}

func randomSerial() (*big.Int, error) {
	return rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
}

// lockFile opens path and takes a flock on it. The lock lasts until the file is closed or the
// process exits, so it cannot outlive a crashed process.
func lockFile(path string, how int) (*os.File, error) {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o644)
	if err != nil {
		return nil, err
	}
	if err := syscall.Flock(int(f.Fd()), how); err != nil {
		f.Close()
		return nil, err
	}
	return f, nil
}
