# Adds conceptory's packages to nixpkgs. Apply it after rust-overlay. It only adds names that
# nixpkgs does not have, because replacing a package would rebuild everything that depends on it
# and miss cache.nixos.org (ADR 0020 "Provide nixpkgs with the overlay and a flake-parts module").
{ crane }:
final: _prev:
let
  build = import ./build.nix {
    pkgs = final;
    inherit crane;
  };
in
{
  inherit (build) conceptory;
  conceptory-ast-grep-rules = build.ast-grep-rules;
}
