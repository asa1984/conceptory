# flake-parts module that sets up a repository's pkgs, packages.ci, and devShell from conceptory
# (ADR 0020 "Provide nixpkgs with the overlay and a flake-parts module").
#
# pkgs is conceptory's nixpkgs with the overlay applied, so the derivations match the ones
# conceptory's CI builds and the binary cache serves them.
{ conceptory }:
{ lib, flake-parts-lib, ... }:
{
  options.perSystem = flake-parts-lib.mkPerSystemOption (
    {
      config,
      pkgs,
      system,
      ...
    }:
    let
      cfg = config.conceptory;
      ciPackages =
        lib.optionals cfg.cli.enable [ pkgs.conceptory ]
        ++ lib.optionals cfg.astGrepRules.enable [ pkgs.conceptory-link-ast-grep-rules ]
        ++ [
          pkgs.ast-grep
          pkgs.actionlint
          pkgs.zizmor
          pkgs.nixfmt
        ]
        ++ cfg.ciPackages;
    in
    {
      options.conceptory = {
        cli.enable = lib.mkEnableOption "the conceptory CLI in packages.ci" // {
          default = true;
        };
        astGrepRules.enable =
          lib.mkEnableOption "linking conceptory's ast-grep rules to .conceptory/ast-grep"
          // {
            default = true;
          };
        ciPackages = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [ ];
          description = "Packages added to packages.ci. The devShell provides them as well.";
        };
        devPackages = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [ ];
          description = "Packages that only the devShell provides.";
        };
        env = lib.mkOption {
          type = lib.types.attrsOf lib.types.str;
          default = { };
          description = "Environment variables set in the devShell.";
        };
        shellHook = lib.mkOption {
          type = lib.types.lines;
          default = "";
          description = "Commands run when entering the devShell.";
        };
      };

      config = {
        _module.args.pkgs = conceptory.legacyPackages.${system};

        # Installed into a Nix profile in CI.
        packages.ci = pkgs.buildEnv {
          name = "ci";
          paths = ciPackages;
        };

        devShells.default = pkgs.mkShell {
          packages = ciPackages ++ cfg.devPackages;
          inherit (cfg) env;
          shellHook = lib.concatLines (
            lib.optional cfg.astGrepRules.enable "conceptory-link-ast-grep-rules" ++ [ cfg.shellHook ]
          );
        };
      };
    }
  );
}
