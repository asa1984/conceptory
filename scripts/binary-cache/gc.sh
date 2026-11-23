#!/usr/bin/env bash
# Deletes the paths in the binary cache on R2 that no recent manifest lists (ADR 0021 "Serve the
# packages conceptory's CI builds from a binary cache on Cloudflare R2").
#
# A manifest is recent if it is newer than the retention period or is the latest for its system.
# Each run first deletes the NARs that no narinfo refers to, and then the narinfos of the paths no
# recent manifest lists. A NAR thus outlives its narinfo by one run, so Nix never finds a narinfo
# whose NAR is gone, even in a cached response.
#
# Expects aws (the AWS CLI) and GNU date on PATH, and the environment variables R2_ENDPOINT,
# R2_BUCKET, AWS_ACCESS_KEY_ID, and AWS_SECRET_ACCESS_KEY.
set -euo pipefail

retention_days=30
cutoff=$(date -u -d "$retention_days days ago" +%Y-%m-%dT%H:%M:%S)

s3api() { aws --endpoint-url "https://$R2_ENDPOINT" s3api "$@"; }
# Prints "<key>\t<last modified>" for each object under the prefix.
list() {
  s3api list-objects-v2 --bucket "$R2_BUCKET" --prefix "$1" \
    --query 'Contents[].[Key, LastModified]' --output text | grep -v '^None$' || true
}
get() { aws --endpoint-url "https://$R2_ENDPOINT" s3 cp "s3://$R2_BUCKET/$1" -; }
delete() {
  local key
  for key in "$@"; do
    echo "delete $key"
    s3api delete-object --bucket "$R2_BUCKET" --key "$key" >/dev/null
  done
}

# Manifest keys end with a UTC timestamp and the revision, so within a system the last key in
# lexical order is the latest.
mapfile -t manifests < <(list manifests/ | sort)
declare -A latest=()
for line in "${manifests[@]}"; do
  key=${line%%$'\t'*}
  system=${key#manifests/}
  latest[${system%%/*}]=$key
done
keep=()
expired=()
for line in "${manifests[@]}"; do
  key=${line%%$'\t'*}
  modified=${line#*$'\t'}
  system=${key#manifests/}
  if [[ $modified > $cutoff || ${latest[${system%%/*}]} == "$key" ]]; then
    keep+=("$key")
  else
    expired+=("$key")
  fi
done

declare -A live=()
for key in "${keep[@]}"; do
  while read -r path; do
    [[ -n $path ]] && live[$(basename "$path" | cut -c1-32)]=1
  done < <(get "$key")
done

mapfile -t narinfos < <(list "" | cut -f1 | grep -E '^[a-z0-9]{32}\.narinfo$' || true)

declare -A referenced=()
for key in "${narinfos[@]}"; do
  url=$(get "$key" | sed -n 's/^URL: //p')
  referenced[$url]=1
done
orphans=()
while read -r key; do
  [[ -n $key && -z ${referenced[$key]:-} ]] && orphans+=("$key")
done < <(list nar/ | cut -f1)
delete "${orphans[@]}"

dead=()
for key in "${narinfos[@]}"; do
  [[ -z ${live[${key%.narinfo}]:-} ]] && dead+=("$key")
done
delete "${dead[@]}"

delete "${expired[@]}"
