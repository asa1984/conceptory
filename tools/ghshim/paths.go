package main

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
)

// paths locates the files ghshim keeps. The state directory outlives a session, so the
// environment snapshot can carry the CA and the git cache. The run directory holds one
// directory per upstream proxy for the daemon that uses it.
//
// The defaults need root. Without write access, `ghshim exec` runs the program without the
// proxy.
type paths struct {
	state string
	run   string
}

func defaultPaths() paths {
	return paths{
		state: envOr("GHSHIM_STATE_DIR", "/var/lib/ghshim"),
		run:   envOr("GHSHIM_RUN_DIR", "/run/ghshim"),
	}
}

func (p paths) caCert() string { return filepath.Join(p.state, "ca.pem") }
func (p paths) caKey() string  { return filepath.Join(p.state, "ca.key") }
func (p paths) repos() string  { return filepath.Join(p.state, "repos") }

// daemon returns the files of the daemon that uses upstream.
func (p paths) daemon(upstream string) daemonPaths {
	sum := sha256.Sum256([]byte(upstream))
	return daemonPaths{dir: filepath.Join(p.run, hex.EncodeToString(sum[:8]))}
}

type daemonPaths struct{ dir string }

func (d daemonPaths) state() string { return filepath.Join(d.dir, "daemon.json") }

// alive is locked by the daemon for as long as it runs.
func (d daemonPaths) alive() string { return filepath.Join(d.dir, "alive") }

// start serializes the processes that start the daemon.
func (d daemonPaths) start() string { return filepath.Join(d.dir, "start") }

// trust holds the CA certificates that verify the upstream proxy.
func (d daemonPaths) trust() string { return filepath.Join(d.dir, "trust.pem") }

// bundle adds ghshim's CA to trust, for the programs that `ghshim exec` starts.
func (d daemonPaths) bundle() string { return filepath.Join(d.dir, "bundle.pem") }

func (d daemonPaths) log() string { return filepath.Join(d.dir, "ghshim.log") }

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

// writeFileAtomic replaces path so that a reader never sees a partial file.
func writeFileAtomic(path string, data []byte, perm os.FileMode) error {
	tmp, err := os.CreateTemp(filepath.Dir(path), ".tmp-*")
	if err != nil {
		return err
	}
	defer os.Remove(tmp.Name())
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Chmod(perm); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmp.Name(), path)
}
