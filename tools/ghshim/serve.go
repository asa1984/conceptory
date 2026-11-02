package main

import (
	"bytes"
	"crypto/x509"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/signal"
	"syscall"
	"time"
)

// daemonState is what a running daemon publishes for `ghshim exec`.
type daemonState struct {
	PID      int    `json:"pid"`
	Addr     string `json:"addr"`
	Upstream string `json:"upstream"`
	Bundle   string `json:"bundle"`
}

func runServe(args []string) error {
	flags := flag.NewFlagSet("serve", flag.ContinueOnError)
	upstreamFlag := flags.String("upstream", "", "URL of the upstream HTTP proxy")
	listen := flags.String("listen", "127.0.0.1:0", "address to listen on")
	if err := flags.Parse(args); err != nil {
		return err
	}
	upstream, err := url.Parse(*upstreamFlag)
	if err != nil || upstream.Scheme != "http" || upstream.Host == "" {
		return fmt.Errorf("--upstream must be an http:// proxy URL, got %q", *upstreamFlag)
	}
	p := defaultPaths()
	logger := log.New(os.Stderr, "ghshim: ", log.LstdFlags)

	ca, err := loadOrCreateAuthority(p)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(p.run, 0o755); err != nil {
		return err
	}
	trustPEM := readTrust(trustFiles(p.bundle()))
	roots := x509.NewCertPool()
	if !roots.AppendCertsFromPEM(trustPEM) {
		return errors.New("found no CA certificates to verify the upstream with")
	}
	bundle := append(bytes.Clone(trustPEM), ca.certPEM...)
	if err := writeFileAtomic(p.bundle(), bundle, 0o644); err != nil {
		return err
	}

	backend := &gitBackend{
		cacheDir: p.repos(),
		remote:   githubRemote,
		env:      append(os.Environ(), "GIT_TERMINAL_PROMPT=0"),
	}
	listener, err := net.Listen("tcp", *listen)
	if err != nil {
		return err
	}
	state, err := json.Marshal(daemonState{
		PID:      os.Getpid(),
		Addr:     listener.Addr().String(),
		Upstream: *upstreamFlag,
		Bundle:   p.bundle(),
	})
	if err != nil {
		return err
	}
	if err := writeFileAtomic(p.daemon(), state, 0o644); err != nil {
		return err
	}
	logger.Printf("listening on %s, upstream %s", listener.Addr(), upstream.Host)

	server := &http.Server{
		Handler:           newProxy(upstream, ca, backend, roots, logger),
		ErrorLog:          logger,
		ReadHeaderTimeout: time.Minute,
	}
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGTERM, syscall.SIGINT)
	go func() {
		<-signals
		server.Close()
	}()
	if err := server.Serve(listener); !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}

// trustFiles lists the CA bundles that the process trusts, leaving out ghshim's own bundle,
// which a nested `ghshim exec` puts in these variables.
func trustFiles(own string) []string {
	var files []string
	seen := map[string]bool{own: true}
	for _, path := range []string{
		os.Getenv("NIX_SSL_CERT_FILE"),
		os.Getenv("SSL_CERT_FILE"),
		"/etc/ssl/certs/ca-certificates.crt",
	} {
		if path != "" && !seen[path] {
			seen[path] = true
			files = append(files, path)
		}
	}
	return files
}

func readTrust(files []string) []byte {
	var out []byte
	for _, path := range files {
		data, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		out = append(out, data...)
		if len(data) > 0 && data[len(data)-1] != '\n' {
			out = append(out, '\n')
		}
	}
	return out
}
