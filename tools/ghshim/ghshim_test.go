package main

import (
	"archive/tar"
	"compress/gzip"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"errors"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
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
		{"github.com", "/hercules-ci/flake-parts/archive/024633cd702b10285db5cb19b40ad48d2399ba60.tar.gz", githubRequest{kindTarball, "hercules-ci", "flake-parts", "024633cd702b10285db5cb19b40ad48d2399ba60"}, true},
		{"github.com", "/o/r.git/archive/refs/tags/v1.0.tar.gz", githubRequest{kindTarball, "o", "r", "refs/tags/v1.0"}, true},
		{"codeload.github.com", "/o/r/tar.gz/refs/heads/main", githubRequest{kindTarball, "o", "r", "refs/heads/main"}, true},
		{"github.com", "/o/r/info/refs", githubRequest{}, false},
		{"github.com", "/o/r/releases/download/v1/x.tar.gz", githubRequest{}, false},
		{"api.github.com", "/repos/o/r/issues", githubRequest{}, false},
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
	state := daemonState{Addr: "127.0.0.1:1234", Upstream: "http://127.0.0.1:9", Bundle: "/run/ghshim/bundle.pem"}
	env := []string{
		"HTTPS_PROXY=http://127.0.0.1:9",
		"SSL_CERT_FILE=/etc/ca.crt",
		"NIX_CONFIG=sandbox = false",
		"GIT_CONFIG_COUNT=1",
		"GIT_CONFIG_KEY_0=core.autocrlf",
		"GIT_CONFIG_VALUE_0=false",
	}
	got := shimEnv(env, state)
	want := map[string]string{
		"HTTPS_PROXY":        "http://127.0.0.1:1234",
		"https_proxy":        "http://127.0.0.1:1234",
		upstreamEnv:          "http://127.0.0.1:9",
		"SSL_CERT_FILE":      "/run/ghshim/bundle.pem",
		"NIX_CONFIG":         "sandbox = false\nssl-cert-file = /run/ghshim/bundle.pem",
		"GIT_CONFIG_COUNT":   "2",
		"GIT_CONFIG_KEY_1":   "http.proxy",
		"GIT_CONFIG_VALUE_1": "http://127.0.0.1:9",
	}
	for key, value := range want {
		if v, _ := lookupEnv(got, key); v != value {
			t.Errorf("%s = %q; want %q", key, v, value)
		}
	}
	if _, ok := lookupEnv(got, "CURL_CA_BUNDLE"); ok {
		t.Error("CURL_CA_BUNDLE is set although it was not before")
	}
	if again := shimEnv(got, state); !slices.Equal(sorted(again), sorted(got)) {
		t.Errorf("shimEnv is not idempotent:\n%v\n%v", sorted(again), sorted(got))
	}
	if upstream := upstreamOf(got); upstream != state.Upstream {
		t.Errorf("upstreamOf = %q; want %q", upstream, state.Upstream)
	}
}

func sorted(env []string) []string {
	out := slices.Clone(env)
	slices.Sort(out)
	return out
}

func TestAuthorityLeaf(t *testing.T) {
	p := paths{state: t.TempDir(), run: t.TempDir()}
	ca, err := loadOrCreateAuthority(p)
	if err != nil {
		t.Fatal(err)
	}
	again, err := loadOrCreateAuthority(p)
	if err != nil {
		t.Fatal(err)
	}
	if !again.cert.Equal(ca.cert) {
		t.Error("loading the CA again created a new one")
	}
	leaf, err := ca.leaf("api.github.com")
	if err != nil {
		t.Fatal(err)
	}
	roots := x509.NewCertPool()
	roots.AddCert(ca.cert)
	if _, err := leaf.Leaf.Verify(x509.VerifyOptions{DNSName: "api.github.com", Roots: roots}); err != nil {
		t.Errorf("leaf does not verify: %v", err)
	}
}

// TestProxyServesFromGit sends requests through the proxy as Nix does and checks that the
// answers come from a local repository.
func TestProxyServesFromGit(t *testing.T) {
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git is not installed")
	}
	origin := filepath.Join(t.TempDir(), "origin")
	runGit(t, "", "init", "--quiet", "--initial-branch=main", origin)
	if err := os.WriteFile(filepath.Join(origin, "flake.nix"), []byte("{ outputs = _: { }; }\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(origin, ".gitattributes"), []byte("ignored export-ignore\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(origin, "ignored"), []byte("not in the archive\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	runGit(t, origin, "add", ".")
	runGit(t, origin, "commit", "--quiet", "--message=init")
	runGit(t, origin, "tag", "--annotate", "--message=v1", "v1")
	sha := strings.TrimSpace(runGit(t, origin, "rev-parse", "HEAD"))
	tree := strings.TrimSpace(runGit(t, origin, "rev-parse", "HEAD^{tree}"))
	commitTime := strings.TrimSpace(runGit(t, origin, "show", "--no-patch", "--format=%ct", "HEAD"))

	p := paths{state: t.TempDir(), run: t.TempDir()}
	ca, err := loadOrCreateAuthority(p)
	if err != nil {
		t.Fatal(err)
	}
	backend := &gitBackend{
		cacheDir: p.repos(),
		remote:   func(owner, repo string) string { return origin },
		env:      gitTestEnv(),
	}
	// Requests that git cannot answer go to the upstream, which no test request reaches.
	upstream, _ := url.Parse("http://127.0.0.1:1")
	server := httptest.NewServer(newProxy(upstream, ca, backend, x509.NewCertPool(), log.New(io.Discard, "", 0)))
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

	for _, ref := range []string{"HEAD", "main", "v1", sha} {
		resp, err := client.Get("https://api.github.com/repos/o/r/commits/" + ref)
		if err != nil {
			t.Fatal(err)
		}
		var body struct {
			SHA    string `json:"sha"`
			Commit struct {
				Tree struct {
					SHA string `json:"sha"`
				} `json:"tree"`
			} `json:"commit"`
		}
		if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
			t.Fatal(err)
		}
		resp.Body.Close()
		if body.SHA != sha || body.Commit.Tree.SHA != tree {
			t.Errorf("commits/%s = %+v; want sha %s and tree %s", ref, body, sha, tree)
		}
	}

	resp, err := client.Get("https://api.github.com/repos/o/r/commits/missing")
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != http.StatusNotFound {
		t.Errorf("commits/missing status = %d; want 404", resp.StatusCode)
	}

	resp, err = client.Get("https://github.com/o/r/archive/" + sha + ".tar.gz")
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("archive status = %d", resp.StatusCode)
	}
	gz, err := gzip.NewReader(resp.Body)
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
		if got := header.ModTime.Unix(); strings.Contains(header.Name, "flake.nix") && strconv.FormatInt(got, 10) != commitTime {
			t.Errorf("mtime of %s = %d; want the commit time %s", header.Name, got, commitTime)
		}
	}
	want := []string{"r-" + sha + "/", "r-" + sha + "/.gitattributes", "r-" + sha + "/flake.nix"}
	if !slices.Equal(names, want) {
		t.Errorf("archive entries = %v; want %v", names, want)
	}
}

func runGit(t *testing.T, dir string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", args...)
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
