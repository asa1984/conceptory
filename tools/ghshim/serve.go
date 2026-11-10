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

// idleTimeout is how long a daemon waits for a connection before it exits. A daemon whose
// upstream proxy went away is never used again.
const idleTimeout = 30 * time.Minute

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
	if err != nil || upstream.Scheme != "http" || upstream.Hostname() == "" {
		return fmt.Errorf("--upstream must be an http:// proxy URL, got %q", *upstreamFlag)
	}
	p := defaultPaths()
	d := p.daemon(*upstreamFlag)
	if err := os.MkdirAll(d.dir, 0o755); err != nil {
		return err
	}
	alive, err := lockFile(d.alive(), syscall.LOCK_EX|syscall.LOCK_NB)
	if err != nil {
		return fmt.Errorf("another daemon for %s is running: %w", *upstreamFlag, err)
	}
	defer alive.Close()
	logger := log.New(os.Stderr, "ghshim: ", log.LstdFlags)

	ca, err := loadOrCreateAuthority(p)
	if err != nil {
		return err
	}
	trustPEM := readTrust(originalTrust(os.Environ()))
	roots := x509.NewCertPool()
	if !roots.AppendCertsFromPEM(trustPEM) {
		return errors.New("found no CA certificates to verify the upstream with")
	}
	if err := writeFileAtomic(d.trust(), trustPEM, 0o644); err != nil {
		return err
	}
	bundle := append(bytes.Clone(trustPEM), ca.certPEM...)
	if err := writeFileAtomic(d.bundle(), bundle, 0o644); err != nil {
		return err
	}

	backend := &gitBackend{cacheDir: p.repos(), remote: githubRemote, env: gitEnv(os.Environ(), d.trust())}
	listener, err := net.Listen("tcp", *listen)
	if err != nil {
		return err
	}
	state, err := json.Marshal(daemonState{
		PID:      os.Getpid(),
		Addr:     listener.Addr().String(),
		Upstream: *upstreamFlag,
		Bundle:   d.bundle(),
	})
	if err != nil {
		return err
	}
	if err := writeFileAtomic(d.state(), state, 0o644); err != nil {
		return err
	}
	logger.Printf("listening on %s, upstream %s", listener.Addr(), upstream.Host)

	handler := newProxy(upstream, ca, backend, roots, logger)
	server := &http.Server{Handler: handler, ErrorLog: logger, ReadHeaderTimeout: time.Minute}
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGTERM, syscall.SIGINT)
	ticker := time.NewTicker(time.Minute)
	defer ticker.Stop()
	go func() {
		for {
			select {
			case <-signals:
				server.Close()
				return
			case <-ticker.C:
				if handler.idleFor() > idleTimeout {
					logger.Printf("idle for %s, exiting", idleTimeout)
					server.Close()
					return
				}
			}
		}
	}()
	if err := server.Serve(listener); !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}

// gitEnv isolates git from the configuration of whoever started the daemon, which could change
// what git archive writes. git reaches GitHub through the upstream proxy in env.
func gitEnv(env []string, trust string) []string {
	out := keepEnv(env, "PATH", "HOME", "TMPDIR", "HTTPS_PROXY", "https_proxy")
	return append(out,
		"GIT_CONFIG_NOSYSTEM=1",
		"GIT_CONFIG_GLOBAL=/dev/null",
		"GIT_TERMINAL_PROMPT=0",
		"GIT_SSL_CAINFO="+trust,
	)
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
