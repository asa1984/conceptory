#!/usr/bin/env bash
# Setup script for Claude Code on the web environments. Paste it into the environment's
# "Setup script" field; it is not run from the repository.
#
# It assumes a session that starts with one repository and adds others later. The script runs
# as root in the directory that holds the clone, and Anthropic snapshots the result when it
# finishes within about five minutes.
set -euo pipefail

# The Anthropic-hosted image ships Nix, but nothing guarantees it stays there.
if [ ! -x /nix/var/nix/profiles/default/bin/nix ]; then
  curl -sSfL https://artifacts.nixos.org/nix-installer |
    sh -s -- install linux --init none --no-confirm \
      --extra-conf 'experimental-features = nix-command flakes'
fi
export PATH=/nix/var/nix/profiles/default/bin:$PATH

# The session's GitHub proxy rejects archive and API requests for repositories that are not
# attached to the session, which is how Nix fetches `github:` inputs. Git over HTTPS still
# works, so every GitHub fetch below goes through it.

# Point the `nixpkgs` registry entry at the nixpkgs the clone locks, so that `nix run nixpkgs#...`
# reuses its source instead of fetching another revision.
nixpkgs="git+https://github.com/NixOS/nixpkgs?ref=nixpkgs-unstable&shallow=1"
for lock in */flake.lock; do
  [ -f "$lock" ] || continue
  # Nix rejects a Git input in the registry that has a commit hash but no branch.
  query=$(jq -r '.nodes as $n | $n.root.inputs.nixpkgs | select(type == "string") | $n[.]
    | select(.locked.type == "github" and .locked.owner == "NixOS" and .locked.repo == "nixpkgs")
    | "ref=\(.original.ref // "nixpkgs-unstable")&rev=\(.locked.rev)"' "$lock")
  if [ -n "$query" ]; then
    nixpkgs="git+https://github.com/NixOS/nixpkgs?$query&shallow=1"
    break
  fi
done
nix registry add nixpkgs "$nixpkgs"

# Fetches the `github:` inputs of a lock file over Git. The NAR hash matches the one in the
# lock file, so Nix finds the store path and does not download the archive. Locking new inputs
# still downloads archives, so `nix flake lock` and `nix flake update` keep failing.
cat >/usr/local/bin/nix-prefetch-lock <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
lock="${1:-.}/flake.lock"
[ -f "$lock" ] || exit 0
jq -r '.nodes[] | .locked // empty | select(.type == "github")
  | [.owner, .repo, .rev, .narHash] | @tsv' "$lock" | sort -u |
  while IFS=$'\t' read -r owner repo rev nar_hash; do
    hash=$(nix hash convert --hash-algo sha256 --to nix32 "$nar_hash")
    [ -e "$(nix-store --print-fixed-path --recursive sha256 "$hash" source)" ] && continue
    echo "nix-prefetch-lock: fetching $owner/$repo@${rev:0:12}" >&2
    nix flake prefetch --quiet "git+https://github.com/$owner/$repo?rev=$rev&shallow=1" >/dev/null
  done
EOF
chmod +x /usr/local/bin/nix-prefetch-lock

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

# Wraps nix-direnv's use_flake so that the inputs are in the store before Nix looks for them.
eval "_nix_direnv_use_flake() $(declare -f use_flake | tail -n +2)"
use_flake() {
  local dir="${1%%#*}"
  nix-prefetch-lock "${dir:-.}" || log_error "nix-prefetch-lock failed"
  _nix_direnv_use_flake "$@"
}
EOF

# Builds the devShell of the clone so that the snapshot holds it. A failure here only makes the
# first SessionStart hook slower, so it must not fail the session.
for envrc in */.envrc; do
  [ -f "$envrc" ] || continue
  (cd "$(dirname "$envrc")" && /usr/local/bin/direnv exec . true) ||
    echo "cloud-setup: failed to build the devShell for $(dirname "$envrc")" >&2
done
