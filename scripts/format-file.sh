#!/usr/bin/env bash
# Formats the given files with the formatter for each extension. Expects to run inside the devShell.
set -euo pipefail

for file in "$@"; do
  case "$file" in
    *.rs) rustfmt "$file" ;;
    *.nix) nixfmt "$file" ;;
    *.toml) taplo fmt "$file" ;;
    *) vp fmt --no-error-on-unmatched-pattern "$file" ;;
  esac
done
