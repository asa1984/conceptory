#!/usr/bin/env bash
# Generates the signing key of the binary cache and saves it as the item binary-cache-signing-key in
# the 1Password vault conceptory-ci (ADR 0021 "Serve the packages conceptory's CI builds from a binary
# cache on Cloudflare R2"). Run it signed in to 1Password with the op CLI.
#
# Usage: scripts/binary-cache/create-signing-key.sh <key name>
# The key name is the cache's domain and the key's generation, such as cache.asa1984.dev-2. When
# replacing the key, archive the existing item first and increment the generation.
#
# The secret key passes only through pipes, never through command arguments or files.
set -euo pipefail

vault=conceptory-ci
item=binary-cache-signing-key
name=${1:?usage: $0 <key name, such as cache.asa1984.dev-2>}

if op item get "$item" --vault "$vault" >/dev/null 2>&1; then
  echo "$vault/$item already exists. Archive it before creating a new key." >&2
  exit 1
fi

secret=$(nix key generate-secret --key-name "$name")
public=$(printf '%s' "$secret" | nix key convert-secret-to-public)

# op reads the item, including the secret key, from a template instead of assignment statements,
# which would put the value in the command arguments.
op item create --vault "$vault" --template <(
  printf '%s' "$secret" | jq --null-input --rawfile secret /dev/stdin \
    --arg title "$item" --arg name "$name" --arg public "$public" '{
      title: $title,
      category: "API_CREDENTIAL",
      fields: [
        { id: "credential", type: "CONCEALED", label: "credential", value: $secret },
        { id: "key_name", type: "STRING", label: "key_name", value: $name },
        { id: "public_key", type: "STRING", label: "public_key", value: $public }
      ]
    }'
) >/dev/null

echo "Saved $vault/$item. Public key:"
echo "$public"
