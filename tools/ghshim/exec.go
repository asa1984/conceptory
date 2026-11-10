package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"slices"
	"strconv"
	"strings"
	"syscall"
	"time"
)

// The variables that `ghshim exec` adds, so that a nested `ghshim exec`, from a program Nix
// starts, can tell what it changed.
const (
	// upstreamEnv is the upstream proxy that the daemon in proxyEnv uses.
	upstreamEnv = "GHSHIM_UPSTREAM"
	// proxyEnv is the daemon that https_proxy pointed at when `ghshim exec` set it.
	proxyEnv = "GHSHIM_PROXY"
	// trustEnv lists the CA bundles the environment trusted before `ghshim exec` replaced
	// them, separated like PATH.
	trustEnv = "GHSHIM_TRUST"
	// bundleEnv is the bundle that `ghshim exec` put in place of those in trustEnv.
	bundleEnv = "GHSHIM_BUNDLE"
)

// caFileVariables are variables that commonly name the CA bundle of the environment. The
// files they name, and any other variable naming one of those files, are replaced by ghshim's
// bundle, so that the programs Nix starts, such as a `nix develop` shell, still verify the
// GitHub hosts when they reach them through the daemon.
var caFileVariables = []string{
	"NIX_SSL_CERT_FILE",
	"SSL_CERT_FILE",
	"CURL_CA_BUNDLE",
	"REQUESTS_CA_BUNDLE",
	"GIT_SSL_CAINFO",
	"NODE_EXTRA_CA_CERTS",
}

// systemBundle is the system's CA bundle, which originalTrust adds to the variables' bundles.
var systemBundle = "/etc/ssl/certs/ca-certificates.crt"

func runExec(args []string) error {
	if len(args) == 0 {
		return errors.New("usage: ghshim exec PROGRAM [ARGS...]")
	}
	program, err := exec.LookPath(args[0])
	if err != nil {
		return err
	}
	env := os.Environ()
	if upstream := upstreamOf(env); upstream != "" {
		state, err := ensureDaemon(defaultPaths(), upstream, env)
		if err != nil {
			// Nix still works for everything the upstream proxy allows.
			fmt.Fprintf(os.Stderr, "ghshim: running without the proxy: %v\n", err)
		} else {
			env = shimEnv(env, state)
		}
	}
	return syscall.Exec(program, args, env)
}

// upstreamOf returns the upstream proxy of env. https_proxy is the upstream unless it still
// points at the daemon that a previous `ghshim exec` set, which happens in programs Nix starts.
func upstreamOf(env []string) string {
	current, _ := lookupEnv(env, "HTTPS_PROXY")
	if current == "" {
		current, _ = lookupEnv(env, "https_proxy")
	}
	if daemon, ok := lookupEnv(env, proxyEnv); ok && daemon == current {
		if upstream, ok := lookupEnv(env, upstreamEnv); ok && upstream != "" {
			return upstream
		}
	}
	return current
}

// ensureDaemon returns the state of the daemon for upstream, starting it if needed. Each
// upstream has its own daemon, so a program that still uses an old upstream never disturbs
// the daemon of the current one.
func ensureDaemon(p paths, upstream string, env []string) (daemonState, error) {
	d := p.daemon(upstream)
	if state, ok := runningDaemon(d, upstream); ok {
		return state, nil
	}
	if err := os.MkdirAll(d.dir, 0o755); err != nil {
		return daemonState{}, err
	}
	start, err := lockFile(d.start(), syscall.LOCK_EX)
	if err != nil {
		return daemonState{}, err
	}
	defer start.Close()
	if state, ok := runningDaemon(d, upstream); ok {
		return state, nil
	}

	self, err := os.Executable()
	if err != nil {
		return daemonState{}, err
	}
	logFile, err := os.OpenFile(d.log(), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return daemonState{}, err
	}
	defer logFile.Close()
	cmd := exec.Command(self, "serve", "--upstream", upstream)
	cmd.Env = daemonEnv(env, upstream)
	cmd.Stdout = logFile
	cmd.Stderr = logFile
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return daemonState{}, err
	}
	pid := cmd.Process.Pid
	exited := make(chan error, 1)
	go func() { exited <- cmd.Wait() }()

	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		select {
		case err := <-exited:
			return daemonState{}, fmt.Errorf("daemon exited: %v (see %s)", err, d.log())
		case <-time.After(50 * time.Millisecond):
		}
		if state, ok := runningDaemon(d, upstream); ok && state.PID == pid {
			return state, nil
		}
	}
	return daemonState{}, fmt.Errorf("daemon did not start in time (see %s)", d.log())
}

// runningDaemon returns the state of the daemon for upstream if one runs. The state file can
// outlive its daemon, for example in an environment snapshot, so only the lock the daemon
// holds counts as a sign of life.
func runningDaemon(d daemonPaths, upstream string) (daemonState, bool) {
	if !locked(d.alive()) {
		return daemonState{}, false
	}
	data, err := os.ReadFile(d.state())
	if err != nil {
		return daemonState{}, false
	}
	var state daemonState
	if json.Unmarshal(data, &state) != nil || state.Upstream != upstream {
		return daemonState{}, false
	}
	conn, err := net.DialTimeout("tcp", state.Addr, time.Second)
	if err != nil {
		return daemonState{}, false
	}
	conn.Close()
	return state, true
}

// locked reports whether another open file holds an exclusive flock on path.
func locked(path string) bool {
	f, err := os.Open(path)
	if err != nil {
		return false
	}
	defer f.Close()
	err = syscall.Flock(int(f.Fd()), syscall.LOCK_SH|syscall.LOCK_NB)
	if err == nil {
		syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
		return false
	}
	return errors.Is(err, syscall.EWOULDBLOCK)
}

// daemonEnv is the environment of the daemon: only what it needs to reach GitHub through the
// upstream proxy and to find its files.
func daemonEnv(env []string, upstream string) []string {
	out := keepEnv(env, "PATH", "HOME", "TMPDIR", "LANG", "GHSHIM_STATE_DIR", "GHSHIM_RUN_DIR")
	return append(out,
		"HTTPS_PROXY="+upstream,
		"https_proxy="+upstream,
		trustEnv+"="+strings.Join(originalTrust(env), string(os.PathListSeparator)),
	)
}

// originalTrust returns the CA bundles that env trusted before any `ghshim exec` changed it.
func originalTrust(env []string) []string {
	if list, ok := lookupEnv(env, trustEnv); ok {
		return slices.DeleteFunc(strings.Split(list, string(os.PathListSeparator)), func(path string) bool {
			return path == ""
		})
	}
	var files []string
	for _, key := range caFileVariables {
		if path, ok := lookupEnv(env, key); ok && path != "" && !slices.Contains(files, path) {
			files = append(files, path)
		}
	}
	if !slices.Contains(files, systemBundle) {
		files = append(files, systemBundle)
	}
	return slices.DeleteFunc(files, func(path string) bool {
		_, err := os.Stat(path)
		return err != nil
	})
}

// shimEnv points env at the daemon in state. Applying it again changes nothing, and applying
// it for another daemon replaces what it set for the previous one.
func shimEnv(env []string, state daemonState) []string {
	trust := originalTrust(env)
	replaced := slices.Clone(trust)
	previous, _ := lookupEnv(env, bundleEnv)
	if previous != "" {
		replaced = append(replaced, previous)
	}
	out := make([]string, 0, len(env)+8)
	for _, entry := range env {
		key, value, _ := strings.Cut(entry, "=")
		if value != "" && !strings.HasPrefix(key, "GHSHIM_") && slices.Contains(replaced, value) {
			entry = key + "=" + state.Bundle
		}
		out = append(out, entry)
	}

	proxyURL := "http://" + state.Addr
	out = setEnv(out, trustEnv, strings.Join(trust, string(os.PathListSeparator)))
	out = setEnv(out, bundleEnv, state.Bundle)
	out = setEnv(out, upstreamEnv, state.Upstream)
	out = setEnv(out, proxyEnv, proxyURL)
	out = setEnv(out, "HTTPS_PROXY", proxyURL)
	out = setEnv(out, "https_proxy", proxyURL)

	// nix.conf may set ssl-cert-file, which takes precedence over NIX_SSL_CERT_FILE.
	config, _ := lookupEnv(out, "NIX_CONFIG")
	var lines []string
	for line := range strings.SplitSeq(config, "\n") {
		if line != "" && line != "ssl-cert-file = "+previous {
			lines = append(lines, line)
		}
	}
	if line := "ssl-cert-file = " + state.Bundle; !slices.Contains(lines, line) {
		lines = append(lines, line)
	}
	out = setEnv(out, "NIX_CONFIG", strings.Join(lines, "\n"))

	// git takes http.proxy over https_proxy, so fetches that Nix runs with git skip the daemon.
	return setGitConfig(out, "http.proxy", state.Upstream)
}

// setGitConfig sets a git setting through GIT_CONFIG_COUNT, replacing an earlier value of key.
func setGitConfig(env []string, key, value string) []string {
	countValue, _ := lookupEnv(env, "GIT_CONFIG_COUNT")
	count, err := strconv.Atoi(countValue)
	if err != nil {
		count = 0
	}
	for i := range count {
		if k, _ := lookupEnv(env, fmt.Sprintf("GIT_CONFIG_KEY_%d", i)); k == key {
			return setEnv(env, fmt.Sprintf("GIT_CONFIG_VALUE_%d", i), value)
		}
	}
	env = setEnv(env, fmt.Sprintf("GIT_CONFIG_KEY_%d", count), key)
	env = setEnv(env, fmt.Sprintf("GIT_CONFIG_VALUE_%d", count), value)
	return setEnv(env, "GIT_CONFIG_COUNT", strconv.Itoa(count+1))
}

// keepEnv returns the entries of env for keys.
func keepEnv(env []string, keys ...string) []string {
	var out []string
	for _, entry := range env {
		if key, _, _ := strings.Cut(entry, "="); slices.Contains(keys, key) {
			out = append(out, entry)
		}
	}
	return out
}

func lookupEnv(env []string, key string) (string, bool) {
	for i := len(env) - 1; i >= 0; i-- {
		if k, v, ok := strings.Cut(env[i], "="); ok && k == key {
			return v, true
		}
	}
	return "", false
}

func setEnv(env []string, key, value string) []string {
	return append(unsetEnv(env, key), key+"="+value)
}

func unsetEnv(env []string, key string) []string {
	out := make([]string, 0, len(env))
	for _, entry := range env {
		if k, _, _ := strings.Cut(entry, "="); k != key {
			out = append(out, entry)
		}
	}
	return out
}
