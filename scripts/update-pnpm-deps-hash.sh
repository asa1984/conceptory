#!/usr/bin/env bash
# Recomputes the hash of the viewer's pnpm dependencies in nix/build.nix, which changes whenever
# pnpm-lock.yaml does. Nix reports the actual hash of a fixed-output derivation whose expected hash
# is empty. Prints the hash.
set -euo pipefail

cd "$(dirname "$0")/.."
file=nix/build.nix

count=$(perl -ne '$n++ if /^\s*hash = "sha256-[^"]*";$/; END { print $n + 0 }' "$file")
if [[ $count != 1 ]]; then
  echo "$file must have exactly one hash, but has $count." >&2
  exit 1
fi

backup=$(mktemp)
cp "$file" "$backup"
trap 'cp "$backup" "$file"; rm -f "$backup"' EXIT

perl -pi -e 's/^(\s*hash = ")sha256-[^"]*(";)$/$1$2/' "$file"
# The build fails because the expected hash is empty, so only its output matters.
got=$(nix build --no-link .#viewer.pnpmDeps 2>&1 | sed -nE 's/^ *got: *(sha256-[A-Za-z0-9+/=]+)$/\1/p' || true)
if [[ -z $got ]]; then
  echo "Nix did not report the hash of the pnpm dependencies." >&2
  exit 1
fi

cp "$backup" "$file"
trap 'rm -f "$backup"' EXIT
HASH=$got perl -pi -e 's/^(\s*hash = ")sha256-[^"]*(";)$/$1$ENV{HASH}$2/' "$file"
echo "$got"
