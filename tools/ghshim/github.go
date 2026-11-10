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

// gitPath is the git that ghshim runs. The Nix package sets it to a store path, so the git on
// the PATH of whoever starts the daemon does not matter.
var gitPath = "git"

type requestKind int

const (
	kindCommit requestKind = iota
	kindTarball
	kindZipball
)

// githubRequest is a request that ghshim can answer from git when GitHub refuses it.
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
	{"api.github.com", regexp.MustCompile(`^/repos/` + namePattern + `/` + namePattern + `/zipball/(.+)$`), kindZipball},
	{"github.com", regexp.MustCompile(`^/` + namePattern + `/` + namePattern + `/archive/(.+)\.tar\.gz$`), kindTarball},
	{"github.com", regexp.MustCompile(`^/` + namePattern + `/` + namePattern + `/archive/(.+)\.zip$`), kindZipball},
	{"codeload.github.com", regexp.MustCompile(`^/` + namePattern + `/` + namePattern + `/tar\.gz/(.+)$`), kindTarball},
	{"codeload.github.com", regexp.MustCompile(`^/` + namePattern + `/` + namePattern + `/zip/(.+)$`), kindZipball},
}

func matchGitHubRequest(host, path string) (githubRequest, bool) {
	for _, route := range routes {
		if route.host != host {
			continue
		}
		m := route.pattern.FindStringSubmatch(path)
		if m == nil {
			continue
		}
		owner, repo, ref := m[1], strings.TrimSuffix(m[2], ".git"), m[3]
		// The names become path components of the git cache, and git would read a ref that
		// starts with a dash as an option.
		if owner == "." || owner == ".." || repo == "" || repo == "." || repo == ".." || strings.HasPrefix(ref, "-") {
			return githubRequest{}, false
		}
		return githubRequest{kind: route.kind, owner: owner, repo: repo, ref: ref}, true
	}
	return githubRequest{}, false
}

var (
	// errRefNotFound means git can read the repository but it has no such ref.
	errRefNotFound = errors.New("ref not found")
	// errUnreadable means git cannot read the repository, for example because it is private.
	errUnreadable = errors.New("repository not readable over git")
	// errNotReproducible means that GitHub's archive of the commit depends on refs that a
	// shallow fetch does not have, so an archive from git would not match it.
	errNotReproducible = errors.New("archive depends on the repository's refs")
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
	out, err := b.git(ctx, dir, "show", "--no-patch", "--format=%T%n%cI", sha)
	if err != nil {
		return commitInfo{}, err
	}
	tree, date, _ := strings.Cut(strings.TrimSpace(string(out)), "\n")
	return commitInfo{sha: sha, tree: tree, date: date}, nil
}

// prepareArchive fetches the commit that ref points to and checks that its archive can match
// GitHub's, so that writeArchive can no longer fail for reasons the client should see as a
// status code.
func (b *gitBackend) prepareArchive(ctx context.Context, owner, repo, ref string) (dir, sha string, err error) {
	if dir, sha, err = b.resolveAndFetch(ctx, owner, repo, ref); err != nil {
		return "", "", err
	}
	if err := b.checkReproducible(ctx, dir, sha); err != nil {
		return "", "", err
	}
	return dir, sha, nil
}

// checkReproducible fails when an export-subst file of the commit uses a placeholder that
// expands to ref names or a description, which depend on refs the shallow fetch lacks.
func (b *gitBackend) checkReproducible(ctx context.Context, dir, sha string) error {
	if found, err := b.grep(ctx, dir, "export-subst", sha, "*.gitattributes", ".gitattributes"); err != nil || !found {
		return err
	}
	found, err := b.grep(ctx, dir, `\$Format:[^$]*%(d|D|\(describe)`, sha)
	if err != nil {
		return err
	}
	if found {
		return fmt.Errorf("%w: %s", errNotReproducible, sha)
	}
	return nil
}

// grep reports whether pattern, an extended regular expression, matches a file of the commit.
func (b *gitBackend) grep(ctx context.Context, dir, pattern, sha string, pathspecs ...string) (bool, error) {
	args := append([]string{"grep", "--quiet", "--extended-regexp", "-e", pattern, sha, "--"}, pathspecs...)
	_, err := b.git(ctx, dir, args...)
	var exit *exec.ExitError
	if errors.As(err, &exit) && exit.ExitCode() == 1 {
		return false, nil
	}
	return err == nil, err
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
	out, err := b.git(ctx, dir, "rev-parse", "--verify", sha+"^{commit}")
	if err != nil {
		return "", "", err
	}
	return dir, strings.TrimSpace(string(out)), nil
}

// writeArchive writes the archive of a fetched commit in format, "tar.gz" or "zip". GitHub
// builds its archives with git archive, so the unpacked contents match, though the compressed
// bytes do not.
func (b *gitBackend) writeArchive(ctx context.Context, w io.Writer, dir, repo, sha, format string) error {
	args := []string{"archive", "--prefix=" + repo + "-" + sha + "/"}
	out := w
	var gz *gzip.Writer
	if format == "zip" {
		args = append(args, "--format=zip")
	} else {
		gz = gzip.NewWriter(w)
		out = gz
		args = append(args, "--format=tar")
	}
	cmd := exec.CommandContext(ctx, gitPath, append(args, sha)...)
	cmd.Dir = dir
	cmd.Env = b.env
	cmd.Stdout = out
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("git archive %s: %w: %s", sha, err, strings.TrimSpace(stderr.String()))
	}
	if gz != nil {
		return gz.Close()
	}
	return nil
}

// resolve returns the object that ref points to, preferring a branch over a tag as GitHub does.
func (b *gitBackend) resolve(ctx context.Context, owner, repo, ref string) (string, error) {
	if fullHash.MatchString(ref) {
		return ref, nil
	}
	out, err := b.git(ctx, "", "ls-remote", b.remote(owner, repo), ref)
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
		if _, err := b.git(ctx, "", "init", "--quiet", "--bare", dir); err != nil {
			return "", err
		}
	}
	if _, err := b.git(ctx, dir, "cat-file", "-e", sha+"^{commit}"); err == nil {
		return dir, nil
	}
	if _, err := b.git(ctx, dir, "fetch", "--quiet", "--depth=1", "--no-tags", b.remote(owner, repo), sha); err != nil {
		return "", fmt.Errorf("%w: %w", errUnreadable, err)
	}
	return dir, nil
}

func (b *gitBackend) git(ctx context.Context, dir string, args ...string) ([]byte, error) {
	cmd := exec.CommandContext(ctx, gitPath, args...)
	cmd.Dir = dir
	cmd.Env = b.env
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("git %s: %w: %s", args[0], err, strings.TrimSpace(stderr.String()))
	}
	return out, nil
}
