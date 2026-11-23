#!/usr/bin/env bash
# Pushes the packages the overlay adds to the binary cache on R2, and records the paths the cache
# should keep for this revision and system in a manifest (ADR 0021 "Serve the packages conceptory's
# CI builds from a binary cache on Cloudflare R2").
#
# Expects aws (the AWS CLI) on PATH and the following environment variables: R2_ENDPOINT (the
# account's S3 API host), R2_BUCKET, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY,
# BINARY_CACHE_SECRET_KEY (the signing key itself), and GITHUB_SHA.
set -euo pipefail

system=$(nix eval --impure --raw --expr builtins.currentSystem)
mapfile -t outputs < <(nix build --no-link --print-out-paths \
  .#conceptory .#ast-grep-rules .#link-ast-grep-rules)
mapfile -t closure < <(nix path-info --recursive "${outputs[@]}")

# cache.nixos.org already serves the paths that come from nixpkgs.
paths=()
for path in "${closure[@]}"; do
  if ! nix path-info --store https://cache.nixos.org "$path" &>/dev/null; then
    paths+=("$path")
  fi
done

key_file=$(mktemp)
trap 'rm -f "$key_file"' EXIT
printf '%s' "$BINARY_CACHE_SECRET_KEY" >"$key_file"

# R2 accepts only its own region names, and the AWS CLI otherwise picks an AWS region.
aws_s3() { aws --endpoint-url "https://$R2_ENDPOINT" --region auto s3 "$@"; }

# Priority 41 makes Nix ask cache.nixos.org (40) first, so lookups of nixpkgs paths do not reach R2.
printf 'StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 41\n' |
  aws_s3 cp - "s3://$R2_BUCKET/nix-cache-info"

# nix copy skips the paths the cache already has and signs the rest as it uploads them.
nix copy \
  --to "s3://$R2_BUCKET?endpoint=$R2_ENDPOINT&region=auto&compression=zstd&secret-key=$key_file" \
  "${paths[@]}"

# The manifest lists every path this revision needs, including those that were already there, so
# garbage collection keeps them.
printf '%s\n' "${paths[@]}" |
  aws_s3 cp - "s3://$R2_BUCKET/manifests/$system/$(date -u +%Y%m%dT%H%M%SZ)-$GITHUB_SHA.txt"
