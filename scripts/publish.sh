#!/usr/bin/env bash
# Builds @asa1984/conceptory as VERSION and publishes it to npm (ADR 0022 "Release with calendar
# versioning and publish the npm package with trusted publishing"). The repository does not record
# the version, so this sets it only in the working tree before packing.
set -euo pipefail

cd "$(dirname "$0")/../packages/conceptory"
npm pkg set version="$VERSION"
vp pack
destination=$(mktemp -d)
pnpm pack --pack-destination "$destination"
tarball=$(echo "$destination"/*.tgz)

packed=$(tar -xzOf "$tarball" package/package.json | jq -r .version)
if [[ $packed != "$VERSION" ]]; then
  echo "The tarball has version $packed instead of $VERSION." >&2
  exit 1
fi

# npm authenticates with the OIDC token of the workflow and adds provenance.
npm publish "$tarball" --access public
