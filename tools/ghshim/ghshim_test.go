package main

import (
	"archive/tar"
	"archive/zip"
	"bytes"
	"compress/gzip"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/json"
	"encoding/pem"
	"errors"
	"io"
	"log"
	"math/big"
	"net"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestMatchGitHubRequest(t *testing.T) {
	tests := []struct {
		host, path string
		want       githubRequest
		ok         bool
	}{
		{"api.github.com", "/repos/NixOS/nixpkgs/commits/nixpkgs-unstable", githubRequest{kindCommit, "NixOS", "nixpkgs", "nixpkgs-unstable"}, true},
		{"api.github.com", "/repos/o/r/commits/refs/heads/feature/x", githubRequest{kindCommit, "o", "r", "refs/heads/feature/x"}, true},
		{"api.github.com", "/repos/o/r/tarball/abc", githubRequest{kindTarball, "o", "r", "abc"}, true},
		{"api.github.com", "/repos/o/r/zipball/abc", githubRequest{kindZipball, "o", "r", "abc"}, true},
		{"github.com", "/hercules-ci/flake-parts/archive/024633cd702b10285db5cb19b40ad48d2399ba60.tar.gz", githubRequest{kindTarball, "hercules-ci", "flake-parts", "024633cd702b10285db5cb19b40ad48d2399ba60"}, true},
		{"github.com", "/o/r.git/archive/refs/tags/v1.0.tar.gz", githubRequest{kindTarball, "o", "r", "refs/tags/v1.0"}, true},
		{"github.com", "/o/r/archive/v1.0.zip", githubRequest{kindZipball, "o", "r", "v1.0"}, true},
		{"codeload.github.com", "/o/r/tar.gz/refs/heads/main", githubRequest{kindTarball, "o", "r", "refs/heads/main"}, true},
		{"codeload.github.com", "/o/r/zip/refs/heads/main", githubRequest{kindZipball, "o", "r", "refs/heads/main"}, true},
		{"github.com", "/o/r/info/refs", githubRequest{}, false},
		{"github.com", "/o/r/releases/download/v1/x.tar.gz", githubRequest{}, false},
		{"api.github.com", "/repos/o/r/issues", githubRequest{}, false},
		{"api.github.com", "/repos/../r/commits/main", githubRequest{}, false},
		{"api.github.com", "/repos/o/../commits/main", githubRequest{}, false},
		{"api.github.com", "/repos/o/r/commits/--upload-pack=x", githubRequest{}, false},
		{"example.com", "/o/r/archive/main.tar.gz", githubRequest{}, false},
	}
	for _, tt := range tests {
		got, ok := matchGitHubRequest(tt.host, tt.path)
		if ok != tt.ok || got != tt.want {
			t.Errorf("matchGitHubRequest(%q, %q) = %+v, %v; want %+v, %v", tt.host, tt.path, got, ok, tt.want, tt.ok)
		}
	}
}

func TestPickRef(t *testing.T) {
	const lsRemote = "1111111111111111111111111111111111111111\tHEAD\n" +
		"2222222222222222222222222222222222222222\trefs/heads/v1\n" +
		"3333333333333333333333333333333333333333\trefs/tags/v1\n" +
		"4444444444444444444444444444444444444444\trefs/tags/v1^{}\n" +
		"5555555555555555555555555555555555555555\trefs/tags/v2\n" +
		"6666666666666666666666666666666666666666\trefs/tags/v2^{}\n" +
		"7777777777777777777777777777777777777777\trefs/tags/v3\n"
	tests := []struct {
		ref, want string
		ok        bool
	}{
		{"HEAD", "1111111111111111111111111111111111111111", true},
		{"v1", "2222222222222222222222222222222222222222", true},
		{"v2", "6666666666666666666666666666666666666666", true},
		{"v3", "7777777777777777777777777777777777777777", true},
		{"refs/tags/v1", "4444444444444444444444444444444444444444", true},
		{"missing", "", false},
	}
	for _, tt := range tests {
		got, ok := pickRef(lsRemote, tt.ref)
		if got != tt.want || ok != tt.ok {
			t.Errorf("pickRef(%q) = %q, %v; want %q, %v", tt.ref, got, ok, tt.want, tt.ok)
		}
	}
}

func TestShimEnv(t *testing.T) {
	// The test must not depend on the machine's own bundle.
	defer func(saved string) { systemBundle = saved }(systemBundle)
	systemBundle = filepath.Join(t.TempDir(), "missing")
	ca := filepath.Join(t.TempDir(), "ca.crt")
	if err := os.WriteFile(ca, []byte("ca"), 0o644); err != nil {
		t.Fatal(err)
	}
	first := daemonState{Addr: "127.0.0.1:1234", Upstream: "http://127.0.0.1:9", Bundle: "/run/ghshim/a/bundle.pem"}
	env := []string{
		"HTTPS_PROXY=http://127.0.0.1:9",
		"SSL_CERT_FILE=" + ca,
		"DENO_CERT=" + ca,
		"EMPTY=",
		"NIX_CONFIG=sandbox = false",
		"GIT_CONFIG_COUNT=1",
		"GIT_CONFIG_KEY_0=core.autocrlf",
		"GIT_CONFIG_VALUE_0=false",
	}
	got := shimEnv(env, first)
	want := map[string]string{
		"HTTPS_PROXY":        "http://127.0.0.1:1234",
		"https_proxy":        "http://127.0.0.1:1234",
		upstreamEnv:          "http://127.0.0.1:9",
		proxyEnv:             "http://127.0.0.1:1234",
		trustEnv:             ca,
		bundleEnv:            first.Bundle,
		"SSL_CERT_FILE":      first.Bundle,
		"DENO_CERT":          first.Bundle,
		"EMPTY":              "",
		"NIX_CONFIG":         "sandbox = false\nssl-cert-file = " + first.Bundle,
		"GIT_CONFIG_COUNT":   "2",
		"GIT_CONFIG_KEY_1":   "http.proxy",
		"GIT_CONFIG_VALUE_1": "http://127.0.0.1:9",
	}
	checkEnv(t, got, want)
	if _, ok := lookupEnv(got, "CURL_CA_BUNDLE"); ok {
		t.Error("CURL_CA_BUNDLE is set although it was not before")
	}
	if again := shimEnv(got, first); !slices.Equal(sorted(again), sorted(got)) {
		t.Errorf("shimEnv is not idempotent:\n%v\n%v", sorted(again), sorted(got))
	}
	if upstream := upstreamOf(got); upstream != first.Upstream {
		t.Errorf("upstreamOf(nested) = %q; want %q", upstream, first.Upstream)
	}

	// The session's proxy moved, and a daemon for the new upstream takes over.
	moved := setEnv(got, "HTTPS_PROXY", "http://127.0.0.1:10")
	if upstream := upstreamOf(moved); upstream != "http://127.0.0.1:10" {
		t.Errorf("upstreamOf(moved) = %q; want the new upstream", upstream)
	}
	second := daemonState{Addr: "127.0.0.1:5678", Upstream: "http://127.0.0.1:10", Bundle: "/run/ghshim/b/bundle.pem"}
	checkEnv(t, shimEnv(moved, second), map[string]string{
		"HTTPS_PROXY":        "http://127.0.0.1:5678",
		upstreamEnv:          "http://127.0.0.1:10",
		"SSL_CERT_FILE":      second.Bundle,
		"DENO_CERT":          second.Bundle,
		"NIX_CONFIG":         "sandbox = false\nssl-cert-file = " + second.Bundle,
		"GIT_CONFIG_COUNT":   "2",
		"GIT_CONFIG_VALUE_1": "http://127.0.0.1:10",
	})
}

func TestDaemonEnv(t *testing.T) {
	env := []string{"PATH=/bin", "HOME=/root", "IN_NIX_SHELL=impure", "GIT_INDEX_FILE=.git/index", "HTTPS_PROXY=http://127.0.0.1:1"}
	got := daemonEnv(env, "http://127.0.0.1:9")
	checkEnv(t, got, map[string]string{"PATH": "/bin", "HOME": "/root", "HTTPS_PROXY": "http://127.0.0.1:9"})
	for _, key := range []string{"IN_NIX_SHELL", "GIT_INDEX_FILE"} {
		if _, ok := lookupEnv(got, key); ok {
			t.Errorf("daemonEnv passes on %s", key)
		}
	}
}

func checkEnv(t *testing.T, env []string, want map[string]string) {
	t.Helper()
	for key, value := range want {
		if v, _ := lookupEnv(env, key); v != value {
			t.Errorf("%s = %q; want %q", key, v, value)
		}
	}
}

func sorted(env []string) []string {
	out := slices.Clone(env)
	slices.Sort(out)
	return out
}

// TestRunningDaemon checks that only the lock of a live daemon counts, never the state file.
func TestRunningDaemon(t *testing.T) {
	p := paths{state: t.TempDir(), run: t.TempDir()}
	d := p.daemon("http://127.0.0.1:9")
	if err := os.MkdirAll(d.dir, 0o755); err != nil {
		t.Fatal(err)
	}
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	// A state file left behind, naming a process that is not the daemon.
	state, _ := json.Marshal(daemonState{PID: os.Getpid(), Addr: listener.Addr().String(), Upstream: "http://127.0.0.1:9"})
	if err := os.WriteFile(d.state(), state, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, ok := runningDaemon(d, "http://127.0.0.1:9"); ok {
		t.Error("a state file without the lock counts as a running daemon")
	}
	alive, err := lockFile(d.alive(), syscall.LOCK_EX|syscall.LOCK_NB)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := runningDaemon(d, "http://127.0.0.1:9"); !ok {
		t.Error("the daemon holding the lock does not count as running")
	}
	if _, ok := runningDaemon(d, "http://127.0.0.1:10"); ok {
		t.Error("a daemon counts as running for another upstream")
	}
	alive.Close()
	if _, ok := runningDaemon(d, "http://127.0.0.1:9"); ok {
		t.Error("the daemon still counts as running after releasing the lock")
	}
}

func TestUpstreamAddr(t *testing.T) {
	for raw, want := range map[string]string{
		"http://proxy":           "proxy:80",
		"http://127.0.0.1:43767": "127.0.0.1:43767",
		"http://[::1]":           "[::1]:80",
	} {
		u, _ := url.Parse(raw)
		if got := upstreamAddr(u); got != want {
			t.Errorf("upstreamAddr(%q) = %q; want %q", raw, got, want)
		}
	}
}

func TestAuthority(t *testing.T) {
	p := paths{state: t.TempDir(), run: t.TempDir()}
	// A CA from before the name constraint gets replaced.
	key, _ := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	template := &x509.Certificate{
		SerialNumber:          big.NewInt(1),
		Subject:               pkix.Name{CommonName: "old"},
		NotBefore:             time.Now().Add(-time.Hour),
		NotAfter:              time.Now().AddDate(10, 0, 0),
		KeyUsage:              x509.KeyUsageCertSign,
		BasicConstraintsValid: true,
		IsCA:                  true,
	}
	der, _ := x509.CreateCertificate(rand.Reader, template, template, &key.PublicKey, key)
	keyDER, _ := x509.MarshalPKCS8PrivateKey(key)
	os.WriteFile(p.caCert(), pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der}), 0o644)
	os.WriteFile(p.caKey(), pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: keyDER}), 0o600)

	ca, err := loadOrCreateAuthority(p)
	if err != nil {
		t.Fatal(err)
	}
	if ca.cert.Subject.CommonName == "old" {
		t.Error("the CA without the name constraint was kept")
	}
	again, err := loadOrCreateAuthority(p)
	if err != nil {
		t.Fatal(err)
	}
	if !again.cert.Equal(ca.cert) {
		t.Error("loading the CA again created a new one")
	}
	roots := x509.NewCertPool()
	roots.AddCert(ca.cert)
	for host, valid := range map[string]bool{"api.github.com": true, "github.com": true, "example.com": false} {
		leaf, err := ca.leaf(host)
		if err != nil {
			t.Fatal(err)
		}
		_, err = leaf.Leaf.Verify(x509.VerifyOptions{DNSName: host, Roots: roots})
		if (err == nil) != valid {
			t.Errorf("leaf for %s verifies: %v; want %v", host, err == nil, valid)
		}
	}
}

// fakeGitHub is a TLS server standing in for the GitHub hosts behind a fake upstream proxy.
// It serves paths that contain /allowed/ and refuses the rest like the session's proxy does.
type fakeGitHub struct {
	upstream *url.URL
	roots    *x509.CertPool
}

func newFakeGitHub(t *testing.T) fakeGitHub {
	t.Helper()
	ca, err := loadOrCreateAuthority(paths{state: t.TempDir(), run: t.TempDir()})
	if err != nil {
		t.Fatal(err)
	}
	github := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.Contains(r.URL.Path, "/allowed/") {
			io.WriteString(w, "from GitHub")
			return
		}
		w.WriteHeader(http.StatusForbidden)
		io.WriteString(w, `{"message":"not enabled for this session"}`)
	}))
	github.TLS = &tls.Config{GetCertificate: func(hello *tls.ClientHelloInfo) (*tls.Certificate, error) {
		return ca.leaf(hello.ServerName)
	}}
	github.StartTLS()
	t.Cleanup(github.Close)

	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		conn, _, err := http.NewResponseController(w).Hijack()
		if err != nil {
			return
		}
		target, err := net.Dial("tcp", github.Listener.Addr().String())
		if err != nil {
			conn.Close()
			return
		}
		io.WriteString(conn, "HTTP/1.1 200 Connection Established\r\n\r\n")
		go func() { io.Copy(target, conn); target.Close() }()
		io.Copy(conn, target)
		conn.Close()
	}))
	t.Cleanup(upstream.Close)
	upstreamURL, _ := url.Parse(upstream.URL)
	roots := x509.NewCertPool()
	roots.AddCert(ca.cert)
	return fakeGitHub{upstream: upstreamURL, roots: roots}
}

// TestProxy sends requests through the proxy as Nix does: GitHub answers what it allows, and
// git answers what it refuses.
func TestProxy(t *testing.T) {
	if _, err := exec.LookPath(gitPath); err != nil {
		t.Skip("git is not installed")
	}
	origin := newOrigin(t, map[string]string{
		"flake.nix":      "{ outputs = _: { }; }\n",
		".gitattributes": "ignored export-ignore\n",
		"ignored":        "not in the archive\n",
	})
	runGit(t, origin, "tag", "--annotate", "--message=v1", "v1")
	sha := strings.TrimSpace(runGit(t, origin, "rev-parse", "HEAD"))
	tree := strings.TrimSpace(runGit(t, origin, "rev-parse", "HEAD^{tree}"))
	commitTime := strings.TrimSpace(runGit(t, origin, "show", "--no-patch", "--format=%ct", "HEAD"))
	subst := newOrigin(t, map[string]string{
		".gitattributes": "version export-subst\n",
		"version":        "$Format:%D$\n",
	})
	substSHA := strings.TrimSpace(runGit(t, subst, "rev-parse", "HEAD"))

	p := paths{state: t.TempDir(), run: t.TempDir()}
	ca, err := loadOrCreateAuthority(p)
	if err != nil {
		t.Fatal(err)
	}
	backend := &gitBackend{
		cacheDir: p.repos(),
		remote: func(owner, repo string) string {
			switch owner {
			case "subst":
				return subst
			case "private":
				return filepath.Join(t.TempDir(), "missing")
			}
			return origin
		},
		env: gitTestEnv(),
	}
	github := newFakeGitHub(t)
	server := httptest.NewServer(newProxy(github.upstream, ca, backend, github.roots, log.New(io.Discard, "", 0)))
	defer server.Close()
	roots := x509.NewCertPool()
	roots.AddCert(ca.cert)
	client := &http.Client{
		Transport: &http.Transport{
			Proxy:           func(*http.Request) (*url.URL, error) { return url.Parse(server.URL) },
			TLSClientConfig: &tls.Config{RootCAs: roots},
		},
		Timeout: time.Minute,
	}
	get := func(rawURL string) (int, []byte) {
		t.Helper()
		resp, err := client.Get(rawURL)
		if err != nil {
			t.Fatal(err)
		}
		defer resp.Body.Close()
		body, err := io.ReadAll(resp.Body)
		if err != nil {
			t.Fatal(err)
		}
		return resp.StatusCode, body
	}

	if status, body := get("https://github.com/o/allowed/archive/" + sha + ".tar.gz"); status != http.StatusOK || string(body) != "from GitHub" {
		t.Errorf("allowed archive = %d %q; want GitHub's answer", status, body)
	}
	if status, _ := get("https://github.com/o/r/info/refs"); status != http.StatusForbidden {
		t.Errorf("unrouted request = %d; want GitHub's 403", status)
	}

	for _, ref := range []string{"HEAD", "main", "v1", sha} {
		status, body := get("https://api.github.com/repos/o/r/commits/" + ref)
		var commit struct {
			SHA    string `json:"sha"`
			Commit struct {
				Tree struct {
					SHA string `json:"sha"`
				} `json:"tree"`
			} `json:"commit"`
		}
		if err := json.Unmarshal(body, &commit); err != nil || status != http.StatusOK {
			t.Fatalf("commits/%s = %d %q", ref, status, body)
		}
		if commit.SHA != sha || commit.Commit.Tree.SHA != tree {
			t.Errorf("commits/%s = %+v; want sha %s and tree %s", ref, commit, sha, tree)
		}
	}
	if status, _ := get("https://api.github.com/repos/o/r/commits/missing"); status != http.StatusNotFound {
		t.Errorf("commits/missing = %d; want 404", status)
	}
	if status, body := get("https://api.github.com/repos/private/r/commits/main"); status != http.StatusForbidden || !strings.Contains(string(body), "not enabled") {
		t.Errorf("unreadable repository = %d %q; want GitHub's refusal", status, body)
	}
	if status, _ := get("https://github.com/subst/r/archive/" + substSHA + ".tar.gz"); status != http.StatusBadGateway {
		t.Errorf("archive depending on refs = %d; want 502", status)
	}

	want := []string{"r-" + sha + "/", "r-" + sha + "/.gitattributes", "r-" + sha + "/flake.nix"}
	status, body := get("https://github.com/o/r/archive/" + sha + ".tar.gz")
	if status != http.StatusOK {
		t.Fatalf("tar.gz archive = %d %q", status, body)
	}
	gz, err := gzip.NewReader(bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	var names []string
	reader := tar.NewReader(gz)
	for {
		header, err := reader.Next()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			t.Fatal(err)
		}
		if header.Typeflag == tar.TypeXGlobalHeader {
			continue
		}
		names = append(names, header.Name)
		if got := strconv.FormatInt(header.ModTime.Unix(), 10); strings.HasSuffix(header.Name, "flake.nix") && got != commitTime {
			t.Errorf("mtime of %s = %s; want the commit time %s", header.Name, got, commitTime)
		}
	}
	if !slices.Equal(names, want) {
		t.Errorf("tar.gz entries = %v; want %v", names, want)
	}

	status, body = get("https://codeload.github.com/o/r/zip/" + sha)
	if status != http.StatusOK {
		t.Fatalf("zip archive = %d %q", status, body)
	}
	archive, err := zip.NewReader(bytes.NewReader(body), int64(len(body)))
	if err != nil {
		t.Fatal(err)
	}
	names = nil
	for _, file := range archive.File {
		names = append(names, file.Name)
	}
	if !slices.Equal(names, want) {
		t.Errorf("zip entries = %v; want %v", names, want)
	}
}

// newOrigin creates a repository with one commit of files on the branch main.
func newOrigin(t *testing.T, files map[string]string) string {
	t.Helper()
	origin := filepath.Join(t.TempDir(), "origin")
	runGit(t, "", "init", "--quiet", "--initial-branch=main", origin)
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(origin, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	runGit(t, origin, "add", ".")
	runGit(t, origin, "commit", "--quiet", "--message=init")
	return origin
}

func runGit(t *testing.T, dir string, args ...string) string {
	t.Helper()
	cmd := exec.Command(gitPath, args...)
	cmd.Dir = dir
	cmd.Env = gitTestEnv()
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v: %s", args, err, out)
	}
	return string(out)
}

func gitTestEnv() []string {
	return append(os.Environ(),
		"GIT_CONFIG_GLOBAL=/dev/null",
		"GIT_CONFIG_NOSYSTEM=1",
		"GIT_AUTHOR_NAME=test",
		"GIT_AUTHOR_EMAIL=test@example.com",
		"GIT_COMMITTER_NAME=test",
		"GIT_COMMITTER_EMAIL=test@example.com",
		"GIT_AUTHOR_DATE=2026-01-02T03:04:05Z",
		"GIT_COMMITTER_DATE=2026-01-02T03:04:05Z",
	)
}
