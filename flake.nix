{
  nixConfig = {
    extra-substituters = [ "https://cache.asa1984.dev" ];
    extra-trusted-public-keys = [ "cache.asa1984.dev-2:ZDLpWsB88DIjBav8I3NSSRuBACUgljt7dP2KVkv7IDg=" ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    inputs@{ self, ... }:
    let
      flakeModule = import ./nix/flake-module.nix { conceptory = self; };
    in
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      # conceptory sets up its own pkgs, packages.ci, and devShell with the module it distributes.
      imports = [ flakeModule ];

      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      flake = {
        # Downstream repositories that need their own nixpkgs apply this instead of using the module
        # (ADR 0020 "Provide nixpkgs with the overlay and a flake-parts module").
        overlays.default = inputs.nixpkgs.lib.composeManyExtensions [
          inputs.rust-overlay.overlays.default
          (import ./nix/overlay.nix { inherit (inputs) crane; })
        ];
        flakeModules.default = flakeModule;
      };

      perSystem =
        {
          config,
          pkgs,
          lib,
          system,
          ...
        }:
        let
          build = import ./nix/build.nix {
            inherit pkgs;
            inherit (inputs) crane;
          };
          # rustfmt.toml uses nightly-only options, so take rustfmt alone from a pinned nightly.
          rustfmt = pkgs.rust-bin.nightly."2026-10-09".rustfmt;
        in
        {
          # nixpkgs with the overlay applied, which the module makes every repository's pkgs. This
          # must not refer to pkgs, which the module takes from here.
          legacyPackages = import inputs.nixpkgs {
            inherit system;
            overlays = [ self.overlays.default ];
          };

          conceptory = {
            # conceptory runs its CLI and rules from the working tree instead.
            cli.enable = false;
            astGrepRules.enable = false;
            ciPackages = [
              pkgs.nodejs_26
              # corepack runs pnpm on its own Node.js, so build it with Node.js 26 instead of the default 24.
              (pkgs.corepack.override { nodejs-slim = pkgs.nodejs-slim_26; })
              # The nightly rustfmt and the toolchain both ship lib/rustlib, so let rustfmt win the collisions.
              (lib.hiPrio rustfmt)
              build.rustToolchain
              # Rust builds need a linker.
              pkgs.stdenv.cc
              pkgs.taplo
              # Provides renovate-config-validator to check the Renovate presets.
              pkgs.renovate
              pkgs.jq
            ];
            env.COREPACK_ENABLE_DOWNLOAD_PROMPT = "0";
            # Make the commands installed by pnpm, such as vp, available without direnv as well.
            shellHook = ''
              export PATH="$PWD/node_modules/.bin:$PATH"
            '';
          };

          packages = {
            inherit (pkgs) conceptory;
            inherit (build) viewer;
            ast-grep-rules = pkgs.conceptory-ast-grep-rules;
            link-ast-grep-rules = pkgs.conceptory-link-ast-grep-rules;
          };

          checks = {
            inherit (config.packages) conceptory;
            renovate =
              pkgs.runCommand "conceptory-renovate-config"
                {
                  nativeBuildInputs = [ pkgs.renovate ];
                  src = lib.fileset.toSource {
                    root = ./.;
                    fileset = lib.fileset.unions [
                      ./renovate
                      ./renovate.json5
                    ];
                  };
                }
                ''
                  cd $src
                  renovate-config-validator --strict --no-global renovate/*.json5 renovate.json5
                  touch $out
                '';
            ast-grep =
              pkgs.runCommand "conceptory-ast-grep-test"
                {
                  nativeBuildInputs = [ pkgs.ast-grep ];
                  src = lib.fileset.toSource {
                    root = ./ast-grep;
                    fileset = lib.fileset.unions [
                      ./ast-grep/sgconfig.yaml
                      ./ast-grep/rules
                      ./ast-grep/rule-tests
                    ];
                  };
                }
                ''
                  cd $src
                  ast-grep test
                  touch $out
                '';
            clippy = build.craneLib.cargoClippy (
              build.commonArgs
              // {
                inherit (build) cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );
            test = build.craneLib.cargoTest (
              build.commonArgs
              // {
                inherit (build) cargoArtifacts;
                cargoTestExtraArgs = "--workspace";
              }
            );

            # The overlay only adds names, because replacing a package in nixpkgs would rebuild its
            # dependents.
            overlay-adds-only =
              let
                added = builtins.attrNames (import ./nix/overlay.nix { inherit (inputs) crane; } pkgs pkgs);
                replaced = builtins.filter (name: inputs.nixpkgs.legacyPackages.${system} ? ${name}) added;
              in
              assert lib.assertMsg (
                replaced == [ ]
              ) "The overlay replaces packages in nixpkgs: ${toString replaced}";
              pkgs.runCommand "conceptory-overlay-adds-only" { } "touch $out";

            # Evaluates a minimal downstream flake that imports the module and builds its outputs.
            flake-module =
              let
                downstream = inputs.flake-parts.lib.mkFlake { inputs.self.outPath = ./.; } {
                  imports = [ self.flakeModules.default ];
                  systems = [ system ];
                  perSystem =
                    { pkgs, ... }:
                    {
                      conceptory.ciPackages = [ pkgs.hello ];
                    };
                };
                ci = downstream.packages.${system}.ci;
                devShell = downstream.devShells.${system}.default;
              in
              pkgs.runCommand "conceptory-flake-module" { } ''
                test -x ${ci}/bin/conceptory
                test -x ${ci}/bin/conceptory-link-ast-grep-rules
                test -x ${ci}/bin/hello
                test -e ${devShell}
                touch $out
              '';
          };
        };
    };
}
