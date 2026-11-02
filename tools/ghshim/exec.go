package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"syscall"
	"time"
)

// upstreamEnv records the upstream proxy once `ghshim exec` has replaced https_proxy, so that
// a nested `ghshim exec` from a program Nix starts does not take the daemon for the upstream.
const upstreamEnv = "GHSHIM_UPSTREAM"

// caFileVariables are the variables through which common tools read their CA bundle. Pointing
// them at ghshim's bundle lets the programs that Nix starts, such as a `nix develop` shell,
// reach the GitHub hosts through the daemon too.
var caFileVariables = []string{
	"SSL_CERT_FILE",
	"NIX_SSL_CERT_FILE",
	"CURL_CA_BUNDLE",
	"REQUESTS_CA_BUNDLE",
	"GIT_SSL_CAINFO",
	"NODE_EXTRA_CA_CERTS",
}

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
		state, err := ensureDaemon(defaultPaths(), upstream)
		if err != nil {
			// Nix still works for everything the upstream proxy allows.
			fmt.Fprintf(os.Stderr, "ghshim: running without the proxy: %v\n", err)
		} else {
			env = shimEnv(env, state)
		}
	}
	return syscall.Exec(program, args, env)
}

func upstreamOf(env []string) string {
	for _, key := range []string{upstreamEnv, "HTTPS_PROXY", "https_proxy"} {
		if v, ok := lookupEnv(env, key); ok && v != "" {
			return v
		}
	}
	return ""
}

// ensureDaemon returns the state of a daemon that uses upstream, starting one if needed. The
// upstream proxy can change within a session, and a daemon for an old one gets replaced.
func ensureDaemon(p paths, upstream string) (daemonState, error) {
	if state, ok := runningDaemon(p, upstream); ok {
		return state, nil
	}
	if err := os.MkdirAll(p.run, 0o755); err != nil {
		return daemonState{}, err
	}
	lock, err := os.OpenFile(p.lock(), os.O_CREATE|os.O_RDWR, 0o644)
	if err != nil {
		return daemonState{}, err
	}
	defer lock.Close()
	if err := syscall.Flock(int(lock.Fd()), syscall.LOCK_EX); err != nil {
		return daemonState{}, err
	}
	if state, ok := runningDaemon(p, upstream); ok {
		return state, nil
	}
	if old, err := readDaemonState(p); err == nil && processAlive(old.PID) {
		syscall.Kill(old.PID, syscall.SIGTERM)
	}
	os.Remove(p.daemon())

	self, err := os.Executable()
	if err != nil {
		return daemonState{}, err
	}
	logFile, err := os.OpenFile(p.logFile(), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return daemonState{}, err
	}
	defer logFile.Close()
	cmd := exec.Command(self, "serve", "--upstream", upstream)
	cmd.Env = daemonEnv(os.Environ(), upstream)
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
			return daemonState{}, fmt.Errorf("daemon exited: %v (see %s)", err, p.logFile())
		case <-time.After(50 * time.Millisecond):
		}
		if state, ok := runningDaemon(p, upstream); ok && state.PID == pid {
			return state, nil
		}
	}
	return daemonState{}, fmt.Errorf("daemon did not start in time (see %s)", p.logFile())
}

func runningDaemon(p paths, upstream string) (daemonState, bool) {
	state, err := readDaemonState(p)
	if err != nil || state.Upstream != upstream || !processAlive(state.PID) {
		return daemonState{}, false
	}
	conn, err := net.DialTimeout("tcp", state.Addr, time.Second)
	if err != nil {
		return daemonState{}, false
	}
	conn.Close()
	return state, true
}

func readDaemonState(p paths) (daemonState, error) {
	var state daemonState
	data, err := os.ReadFile(p.daemon())
	if err != nil {
		return state, err
	}
	err = json.Unmarshal(data, &state)
	return state, err
}

// processAlive reports whether pid runs. A daemon is a child of the program that the first
// `ghshim exec` became, so a daemon that died before that program shows up as a zombie.
func processAlive(pid int) bool {
	if pid <= 0 || syscall.Kill(pid, 0) != nil {
		return false
	}
	stat, err := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
	if err != nil {
		return true
	}
	// The state follows the command name, which is in parentheses and may contain spaces.
	if i := strings.LastIndexByte(string(stat), ')'); i >= 0 && i+2 < len(stat) {
		return stat[i+2] != 'Z'
	}
	return true
}

// daemonEnv is the environment of the daemon, which reaches GitHub through the upstream proxy.
func daemonEnv(env []string, upstream string) []string {
	env = setEnv(env, "HTTPS_PROXY", upstream)
	env = setEnv(env, "https_proxy", upstream)
	return unsetEnv(env, upstreamEnv)
}

// shimEnv points env at the daemon. Applying it again changes nothing.
func shimEnv(env []string, state daemonState) []string {
	proxyURL := "http://" + state.Addr
	if _, ok := lookupEnv(env, upstreamEnv); !ok {
		env = setEnv(env, upstreamEnv, state.Upstream)
	}
	env = setEnv(env, "HTTPS_PROXY", proxyURL)
	env = setEnv(env, "https_proxy", proxyURL)

	// nix.conf may set ssl-cert-file, which takes precedence over NIX_SSL_CERT_FILE.
	line := "ssl-cert-file = " + state.Bundle
	if config, _ := lookupEnv(env, "NIX_CONFIG"); !strings.Contains(config, line) {
		if config != "" {
			config += "\n"
		}
		env = setEnv(env, "NIX_CONFIG", config+line)
	}
	for _, key := range caFileVariables {
		if _, ok := lookupEnv(env, key); ok {
			env = setEnv(env, key, state.Bundle)
		}
	}
	// git takes http.proxy over https_proxy, so fetches that Nix runs with git skip the daemon.
	return addGitConfig(env, "http.proxy", state.Upstream)
}

// addGitConfig adds a git setting through GIT_CONFIG_COUNT unless one for key is there.
func addGitConfig(env []string, key, value string) []string {
	countValue, _ := lookupEnv(env, "GIT_CONFIG_COUNT")
	count, err := strconv.Atoi(countValue)
	if err != nil {
		count = 0
	}
	for i := range count {
		if k, _ := lookupEnv(env, fmt.Sprintf("GIT_CONFIG_KEY_%d", i)); k == key {
			return env
		}
	}
	env = setEnv(env, fmt.Sprintf("GIT_CONFIG_KEY_%d", count), key)
	env = setEnv(env, fmt.Sprintf("GIT_CONFIG_VALUE_%d", count), value)
	return setEnv(env, "GIT_CONFIG_COUNT", strconv.Itoa(count+1))
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
