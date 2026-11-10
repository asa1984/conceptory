#!/usr/bin/env bash
# Setup script for Claude Code on the web environments. Paste it into the environment's
# "Setup script" field; it is not run from the repository.
#
# It assumes a session that starts with one repository and adds others later. The script runs
# as root in the directory that holds the clone, and Anthropic snapshots the result when it
# finishes within about five minutes. A failing script stops the session from starting, so
# only a missing Nix fails it; without ghshim or the devShell, the session starts without them.
set -euo pipefail

ghshim_flake="${GHSHIM_FLAKE:-git+https://github.com/asa1984/conceptory?ref=main&shallow=1}"

if [ ! -x /nix/var/nix/profiles/default/bin/nix ]; then
  curl -sSfL https://artifacts.nixos.org/nix-installer |
    sh -s -- install linux --init none --no-confirm \
      --extra-conf 'experimental-features = nix-command flakes'
fi
export PATH=/nix/var/nix/profiles/default/bin:$PATH
profile=/nix/var/nix/profiles/default

# Fetches the locked `github:` inputs of a flake over git, for the build of ghshim itself. The
# NAR hash matches the lock file, so Nix finds the store path and does not download the archive.
prefetch_lock() {
  [ -f "$1/flake.lock" ] || return 0
  jq -r '.nodes[] | .locked // empty | select(.type == "github")
    | [.owner, .repo, .rev, .narHash] | @tsv' "$1/flake.lock" | sort -u |
    while IFS=$'\t' read -r owner repo rev nar_hash; do
      hash=$(nix hash convert --hash-algo sha256 --to nix32 "$nar_hash")
      [ -e "$(nix-store --print-fixed-path --recursive sha256 "$hash" source)" ] && continue
      nix flake prefetch --quiet "git+https://github.com/$owner/$repo?rev=$rev&shallow=1" >/dev/null
    done
}

# Builds ghshim and shadows every Nix command with a wrapper that runs it through ghshim.
# Each step checks its result, because `set -e` does not apply inside a function that a
# condition calls.
install_ghshim() {
  local source nix_bin wrappers wrapper command
  source=$(nix flake prefetch --json "$ghshim_flake" | jq -r .storePath) || return 1
  prefetch_lock "$source" || return 1
  mkdir -p /var/lib/ghshim
  nix build --out-link /var/lib/ghshim/package "path:$source#ghshim" || return 1
  /var/lib/ghshim/package/bin/ghshim init || return 1

  nix_bin=$(nix profile list --profile "$profile" --json | jq -r '.elements.nix.storePaths[0] // empty')/bin
  if [ ! -x "$nix_bin/nix" ]; then
    echo "cloud-setup: found no nix package in $profile" >&2
    return 1
  fi
  # Nix commands dispatch on their name, so each one gets a wrapper that keeps it.
  wrappers=$(mktemp -d)
  mkdir "$wrappers/bin"
  for command in "$nix_bin"/*; do
    printf '#!/bin/sh\nexec /var/lib/ghshim/package/bin/ghshim exec %s "$@"\n' "$command" \
      >"$wrappers/bin/$(basename "$command")"
    chmod +x "$wrappers/bin/$(basename "$command")"
  done
  wrapper=$(nix store add --name ghshim-nix "$wrappers") || return 1
  rm -rf "$wrappers"
  # A higher priority lets the wrappers shadow the commands of the nix package in the profile.
  nix profile remove --profile "$profile" ghshim-nix 2>/dev/null || true
  nix profile add --profile "$profile" --priority 4 "$wrapper" || return 1
  # The image also installs Nix into root's nix-env profile, which a devShell's PATH lists before
  # the default profile. nix-env takes a priority only for installed packages, so lower the nix
  # package's instead.
  if [ -x ~/.nix-profile/bin/nix ] && [ -f ~/.nix-profile/manifest.nix ]; then
    nix-env --uninstall ghshim-nix 2>/dev/null || true
    nix-env --set-flag priority 6 nix || return 1
    nix-env --install "$wrapper" || return 1
  fi
}
if ! install_ghshim; then
  echo "cloud-setup: continuing without ghshim; github: inputs outside the session fail" >&2
fi

# Points the `nixpkgs` registry entry at the source of the nixpkgs the clone locks, so that
# `nix run nixpkgs#...` neither resolves nor downloads another revision.
for lock in */flake.lock; do
  [ -f "$lock" ] || continue
  ref=$(jq -r '.nodes as $n | $n.root.inputs.nixpkgs | select(type == "string") | $n[.].locked
    | select(.type == "github" and .owner == "NixOS" and .repo == "nixpkgs")
    | "github:NixOS/nixpkgs/\(.rev)?narHash=\(.narHash | @uri)"' "$lock") || continue
  if [ -n "$ref" ] && source=$(nix flake prefetch --json "$ref" | jq -r .storePath); then
    nix registry add nixpkgs "path:$source" || true
    break
  fi
done

# Builds direnv and nix-direnv on first use instead of installing them into a profile.
cat >/usr/local/bin/direnv <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
root=/var/cache/cc-direnv
if [ ! -x "$root/direnv/bin/direnv" ] || [ ! -e "$root/nix-direnv" ]; then
  mkdir -p "$root"
  exec 9>"$root.lock"
  flock 9
  nix build --out-link "$root/direnv" nixpkgs#direnv >&2
  nix build --out-link "$root/nix-direnv" nixpkgs#nix-direnv >&2
  exec 9>&-
fi
exec "$root/direnv/bin/direnv" "$@"
EOF
chmod +x /usr/local/bin/direnv

mkdir -p ~/.config/direnv
# The clones live under /home/user, and an added repository has no one to run `direnv allow`.
cat >~/.config/direnv/direnv.toml <<'EOF'
[global]
hide_env_diff = true
warn_timeout = "5m"

[whitelist]
prefix = ["/home/user"]
EOF
cat >~/.config/direnv/direnvrc <<'EOF'
source /var/cache/cc-direnv/nix-direnv/share/nix-direnv/direnvrc
EOF

# Builds the devShell of the clone so that the snapshot holds it. The time limit keeps the
# script within the snapshot's budget; the SessionStart hook finishes an unfinished build.
for envrc in */.envrc; do
  [ -f "$envrc" ] || continue
  dir=$(dirname "$envrc")
  (cd "$dir" && timeout 180 /usr/local/bin/direnv exec . true) ||
    echo "cloud-setup: did not finish the devShell of $dir" >&2
done
