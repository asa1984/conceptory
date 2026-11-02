package main

import (
	"bufio"
	"bytes"
	"compress/gzip"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
)

// githubHosts are the hosts whose TLS ghshim terminates.
var githubHosts = map[string]bool{
	"github.com":          true,
	"api.github.com":      true,
	"codeload.github.com": true,
}

type requestKind int

const (
	kindCommit requestKind = iota
	kindTarball
)

// githubRequest is a request that ghshim answers from git instead of forwarding.
type githubRequest struct {
	kind  requestKind
	owner string
	repo  string
	ref   string
}

const namePattern = `([A-Za-z0-9_.-]+)`

var routes = []struct {
	host    string
	pattern *regexp.Regexp
	kind    requestKind
}{
	// Nix resolves a ref through the commits API and then downloads the archive of the commit.
	{"api.github.com", regexp.MustCompile(`^/repos/` + namePattern + `/` + namePattern + `/commits/(.+)$`), kindCommit},
	{"api.github.com", regexp.MustCompile(`^/repos/` + namePattern + `/` + namePattern + `/tarball/(.+)$`), kindTarball},
	{"github.com", regexp.MustCompile(`^/` + namePattern + `/` + namePattern + `/archive/(.+)\.tar\.gz$`), kindTarball},
	{"codeload.github.com", regexp.MustCompile(`^/` + namePattern + `/` + namePattern + `/tar\.gz/(.+)$`), kindTarball},
}

func matchGitHubRequest(host, path string) (githubRequest, bool) {
	for _, route := range routes {
		if route.host != host {
			continue
		}
		if m := route.pattern.FindStringSubmatch(path); m != nil {
			return githubRequest{kind: route.kind, owner: m[1], repo: strings.TrimSuffix(m[2], ".git"), ref: m[3]}, true
		}
	}
	return githubRequest{}, false
}

var (
	// errRefNotFound means git can read the repository but it has no such ref.
	errRefNotFound = errors.New("ref not found")
	// errUnreadable means git cannot read the repository, for example because it is private.
	// GitHub then gets to answer the request itself.
	errUnreadable = errors.New("repository not readable over git")
)

var fullHash = regexp.MustCompile(`^[0-9a-f]{40}$`)

// gitBackend answers GitHub requests from shallow fetches kept in a bare repository per
// GitHub repository.
type gitBackend struct {
	cacheDir string
	// remote returns the URL that git fetches a repository from.
	remote func(owner, repo string) string
	env    []string

	locks sync.Map
}

func githubRemote(owner, repo string) string {
	return fmt.Sprintf("https://github.com/%s/%s", owner, repo)
}

type commitInfo struct {
	sha  string
	tree string
	date string
}

func (b *gitBackend) commit(ctx context.Context, owner, repo, ref string) (commitInfo, error) {
	dir, sha, err := b.resolveAndFetch(ctx, owner, repo, ref)
	if err != nil {
		return commitInfo{}, err
	}
	out, err := b.git(ctx, dir, nil, "show", "--no-patch", "--format=%T%n%cI", sha)
	if err != nil {
		return commitInfo{}, err
	}
	tree, date, _ := strings.Cut(strings.TrimSpace(string(out)), "\n")
	return commitInfo{sha: sha, tree: tree, date: date}, nil
}

// prepareArchive resolves ref and fetches it, so that writeArchive can no longer fail for
// reasons the client should see as a status code.
func (b *gitBackend) prepareArchive(ctx context.Context, owner, repo, ref string) (dir, sha string, err error) {
	return b.resolveAndFetch(ctx, owner, repo, ref)
}

// resolveAndFetch fetches the commit that ref points to. git ls-remote does not list the
// commit behind an annotated tag for a pattern, so the tag is peeled after the fetch.
func (b *gitBackend) resolveAndFetch(ctx context.Context, owner, repo, ref string) (dir, sha string, err error) {
	if sha, err = b.resolve(ctx, owner, repo, ref); err != nil {
		return "", "", err
	}
	if dir, err = b.fetch(ctx, owner, repo, sha); err != nil {
		return "", "", err
	}
	out, err := b.git(ctx, dir, nil, "rev-parse", "--verify", sha+"^{commit}")
	if err != nil {
		return "", "", err
	}
	return dir, strings.TrimSpace(string(out)), nil
}

// writeArchive writes the gzipped tarball of a fetched commit. GitHub builds its archives with
// git archive, so the contents, including export-ignore and export-subst, match.
func (b *gitBackend) writeArchive(ctx context.Context, w io.Writer, dir, repo, sha string) error {
	gz := gzip.NewWriter(w)
	cmd := exec.CommandContext(ctx, "git", "archive", "--format=tar", "--prefix="+repo+"-"+sha+"/", sha)
	cmd.Dir = dir
	cmd.Env = b.env
	cmd.Stdout = gz
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("git archive %s: %w: %s", sha, err, strings.TrimSpace(stderr.String()))
	}
	return gz.Close()
}

// resolve returns the object that ref points to, preferring a branch over a tag as GitHub does.
func (b *gitBackend) resolve(ctx context.Context, owner, repo, ref string) (string, error) {
	if fullHash.MatchString(ref) {
		return ref, nil
	}
	out, err := b.git(ctx, "", nil, "ls-remote", b.remote(owner, repo), ref)
	if err != nil {
		return "", fmt.Errorf("%w: %w", errUnreadable, err)
	}
	if sha, ok := pickRef(string(out), ref); ok {
		return sha, nil
	}
	return "", fmt.Errorf("%w: %s/%s %s", errRefNotFound, owner, repo, ref)
}

// pickRef finds ref in the output of git ls-remote. An annotated tag resolves to the commit
// it points to.
func pickRef(lsRemote, ref string) (string, bool) {
	refs := map[string]string{}
	scanner := bufio.NewScanner(strings.NewReader(lsRemote))
	for scanner.Scan() {
		if sha, name, ok := strings.Cut(scanner.Text(), "\t"); ok {
			refs[name] = sha
		}
	}
	candidates := []string{"refs/heads/" + ref, "refs/tags/" + ref + "^{}", "refs/tags/" + ref}
	if ref == "HEAD" || strings.HasPrefix(ref, "refs/") {
		candidates = []string{ref + "^{}", ref}
	}
	for _, name := range candidates {
		if sha, ok := refs[name]; ok {
			return sha, true
		}
	}
	return "", false
}

func (b *gitBackend) fetch(ctx context.Context, owner, repo, sha string) (string, error) {
	dir := filepath.Join(b.cacheDir, strings.ToLower(owner), strings.ToLower(repo)+".git")
	lock, _ := b.locks.LoadOrStore(dir, &sync.Mutex{})
	lock.(*sync.Mutex).Lock()
	defer lock.(*sync.Mutex).Unlock()

	if _, err := os.Stat(dir); errors.Is(err, os.ErrNotExist) {
		if err := os.MkdirAll(filepath.Dir(dir), 0o755); err != nil {
			return "", err
		}
		if _, err := b.git(ctx, "", nil, "init", "--quiet", "--bare", dir); err != nil {
			return "", err
		}
	}
	if _, err := b.git(ctx, dir, nil, "cat-file", "-e", sha+"^{commit}"); err == nil {
		return dir, nil
	}
	if _, err := b.git(ctx, dir, nil, "fetch", "--quiet", "--depth=1", "--no-tags", b.remote(owner, repo), sha); err != nil {
		return "", fmt.Errorf("%w: %w", errUnreadable, err)
	}
	return dir, nil
}

func (b *gitBackend) git(ctx context.Context, dir string, stdin io.Reader, args ...string) ([]byte, error) {
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = dir
	cmd.Env = b.env
	cmd.Stdin = stdin
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("git %s: %w: %s", args[0], err, strings.TrimSpace(stderr.String()))
	}
	return out, nil
}
