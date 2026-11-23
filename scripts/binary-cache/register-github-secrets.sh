#!/usr/bin/env bash
# Copies the binary cache's credentials from the 1Password vault conceptory-ci to the secrets and
# variables the workflow binary-cache reads (ADR 0021 "Serve the packages conceptory's CI builds
# from a binary cache on Cloudflare R2"). 1Password holds the values, so run this again after
# changing them there. Run it signed in to 1Password with the op CLI and to GitHub with gh.
#
# | GitHub                               | 1Password (vault conceptory-ci)     |
# | ------------------------------------ | ----------------------------------- |
# | secret BINARY_CACHE_SECRET_KEY (env) | binary-cache-signing-key/credential |
# | secret R2_ACCESS_KEY_ID (env)        | r2-binary-cache/username            |
# | secret R2_SECRET_ACCESS_KEY (env)    | r2-binary-cache/credential          |
# | variable R2_ENDPOINT (env)           | r2-binary-cache/endpoint            |
# | variable R2_BUCKET (env)             | r2-binary-cache/bucket              |
#
# (env) means the environment binary-cache, which must exist already. The public key is not a
# secret, so the action setup-nix has it in the repository instead.
#
# Values pass only through pipes, never through command arguments or files.
set -euo pipefail

repo=asa1984/conceptory
environment=binary-cache
vault=conceptory-ci

# Reads the value before calling gh, so a failed read never sets an empty value.
read_item() { op read --no-newline "op://$vault/$1"; }

secret() {
  local value
  value=$(read_item "$2")
  printf '%s' "$value" | gh secret set "$1" --repo "$repo" --env "$environment"
}

variable() {
  local value
  value=$(read_item "$2")
  printf '%s' "$value" | gh variable set "$1" --repo "$repo" --env "$environment"
}

secret BINARY_CACHE_SECRET_KEY binary-cache-signing-key/credential
secret R2_ACCESS_KEY_ID r2-binary-cache/username
secret R2_SECRET_ACCESS_KEY r2-binary-cache/credential
variable R2_ENDPOINT r2-binary-cache/endpoint
variable R2_BUCKET r2-binary-cache/bucket
