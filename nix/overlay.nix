# Adds conceptory's packages to nixpkgs. Apply it after rust-overlay. It only adds names that
# nixpkgs does not have, because replacing a package would rebuild everything that depends on it
# and miss cache.nixos.org (ADR 0020 "Provide nixpkgs with the overlay and a flake-parts module").
{ crane, buildInfo }:
final: _prev:
let
  build = import ./build.nix {
    pkgs = final;
    inherit crane buildInfo;
  };
in
{
  inherit (build) conceptory;
  conceptory-ast-grep-rules = build.ast-grep-rules;
  # Links the ast-grep rules to the path sgconfig.yaml refers to (ADR 0011 "Distribute ast-grep
  # rules by language via the Nix flake"). CI runs it after installing packages.ci.
  conceptory-link-ast-grep-rules = final.writeShellApplication {
    name = "conceptory-link-ast-grep-rules";
    text = ''
      mkdir -p .conceptory
      ln -sfn ${final.conceptory-ast-grep-rules} .conceptory/ast-grep
    '';
  };
}
