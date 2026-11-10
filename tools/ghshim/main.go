// Command ghshim lets Nix fetch `github:` inputs in a sandbox whose egress proxy refuses
// GitHub archive and API requests for repositories outside the session but allows git.
//
// `ghshim exec` wraps Nix. It starts the proxy daemon when needed and runs the program with
// https_proxy pointing at the daemon. The daemon terminates TLS for the GitHub hosts with its
// own CA and forwards their requests to GitHub. When GitHub refuses an archive or commit
// request, the daemon answers it from a shallow git fetch. Other hosts are tunnelled to the
// upstream proxy unchanged.
//
// Archives in other formats than tar.gz and zip, and release assets, are not in git, so they
// keep getting GitHub's refusal.
package main

import (
	"fmt"
	"os"
)

const usage = `usage:
  ghshim exec PROGRAM [ARGS...]  run PROGRAM through the proxy, starting it if needed
  ghshim serve --upstream URL    run the proxy in the foreground
  ghshim init                    create the CA ahead of the first run`

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, usage)
		os.Exit(2)
	}
	var err error
	switch os.Args[1] {
	case "exec":
		err = runExec(os.Args[2:])
	case "serve":
		err = runServe(os.Args[2:])
	case "init":
		_, err = loadOrCreateAuthority(defaultPaths())
	default:
		fmt.Fprintln(os.Stderr, usage)
		os.Exit(2)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "ghshim:", err)
		os.Exit(1)
	}
}
