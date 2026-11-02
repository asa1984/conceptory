package main

import (
	"os"
	"path/filepath"
)

// paths locates the files ghshim keeps. The state directory outlives a session, so the
// environment snapshot can carry the CA and the git cache; the run directory belongs to the
// daemon of the current session.
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

func (p paths) caCert() string  { return filepath.Join(p.state, "ca.pem") }
func (p paths) caKey() string   { return filepath.Join(p.state, "ca.key") }
func (p paths) repos() string   { return filepath.Join(p.state, "repos") }
func (p paths) daemon() string  { return filepath.Join(p.run, "daemon.json") }
func (p paths) lock() string    { return filepath.Join(p.run, "lock") }
func (p paths) bundle() string  { return filepath.Join(p.run, "bundle.pem") }
func (p paths) logFile() string { return filepath.Join(p.run, "ghshim.log") }

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
