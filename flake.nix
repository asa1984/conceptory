{
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
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      perSystem =
        {
          config,
          pkgs,
          lib,
          system,
          ...
        }:
        let
          # Use the same toolchain for the devShell and the package builds.
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
          # conceptory_okf embeds the schemas with include_str!. The clippy check reads clippy.toml.
          commonArgs = {
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./clippy.toml
                ./crates
                ./schemas
              ];
            };
            pname = "conceptory";
            version = (lib.importTOML ./Cargo.toml).workspace.package.version;
            strictDeps = true;
          };
          # Build only the dependencies first and share them between the package and the checks.
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # rustfmt.toml uses nightly-only options, so take rustfmt alone from a pinned nightly.
          rustfmt = pkgs.rust-bin.nightly."2026-10-09".rustfmt;
          # Tools for development and CI. The devShell and packages.tools both provide them.
          tools = [
            pkgs.nodejs_26
            # corepack runs pnpm on its own Node.js, so build it with Node.js 26 instead of the default 24.
            (pkgs.corepack.override { nodejs-slim = pkgs.nodejs-slim_26; })
            # The nightly rustfmt and the toolchain both ship lib/rustlib, so let rustfmt win the collisions.
            (lib.hiPrio rustfmt)
            rustToolchain
            pkgs.taplo
            pkgs.ast-grep
            pkgs.actionlint
            pkgs.zizmor
            pkgs.cocogitto
            # Provides renovate-config-validator to check the Renovate presets.
            pkgs.renovate
            pkgs.nixfmt
            pkgs.jq
          ];
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          # The viewer SPA embedded in the conceptory CLI.
          packages.viewer = pkgs.stdenv.mkDerivation (finalAttrs: {
            pname = "conceptory-viewer";
            version = (lib.importTOML ./Cargo.toml).workspace.package.version;
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./package.json
                ./pnpm-lock.yaml
                ./pnpm-workspace.yaml
                ./vite.config.ts
                ./packages/tsconfig
                ./packages/vite-plus-config
                ./viewer/package.json
                ./viewer/index.html
                ./viewer/tsconfig.json
                ./viewer/vite.config.ts
                ./viewer/src
              ];
            };
            pnpmDeps = pkgs.fetchPnpmDeps {
              inherit (finalAttrs) pname version src;
              pnpm = pkgs.pnpm_12;
              fetcherVersion = 4;
              # pnpm leaves an empty v11/links in the store on darwin but not on Linux, which changes the hash.
              # Offline installs only need v11/files and the index.
              preFixup = ''
                rm -rf "$storePath/v11/links"
              '';
              hash = "sha256-h2o21nLmTADAtgtIemFoVigtLFnEYa6QkTFeayD/5/0=";
            };
            nativeBuildInputs = [
              pkgs.nodejs_26
              pkgs.pnpm_12
              pkgs.pnpmConfigHook
            ]
            # Point the native binaries distributed via npm at Nix's dynamic linker and libraries.
            ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.autoPatchelfHook ];
            buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.stdenv.cc.cc.lib ];
            buildPhase = ''
              runHook preBuild
              cd viewer
              ../node_modules/.bin/vp build
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              cp -r dist $out
              runHook postInstall
            '';
          });

          packages.conceptory = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "--locked --package conceptory";
              # Tests run in checks.
              doCheck = false;
              env.CONCEPTORY_VIEWER_DIST = config.packages.viewer;
              meta.mainProgram = "conceptory";
            }
          );

          # ast-grep rules distributed to downstream repositories, with one directory per language. Each
          # repository links to this package from its devShell and points sgconfig.yaml at the directories
          # for the languages it uses.
          packages.ast-grep-rules = pkgs.runCommand "conceptory-ast-grep-rules" { } ''
            cp -r ${
              lib.fileset.toSource {
                root = ./ast-grep/rules;
                fileset = ./ast-grep/rules;
              }
            } $out
          '';

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
            clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );
            test = craneLib.cargoTest (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoTestExtraArgs = "--workspace";
              }
            );
          };

          # Installed into a Nix profile in CI. Rust builds need a linker, so it includes a C compiler.
          packages.tools = pkgs.buildEnv {
            name = "conceptory-tools";
            paths = tools ++ [ pkgs.stdenv.cc ];
          };

          # Rust builds need a linker, so use mkShell, which includes a C compiler.
          devShells.default = pkgs.mkShell {
            packages = tools;
            env = {
              COREPACK_ENABLE_DOWNLOAD_PROMPT = "0";
            };
            # Make the commands installed by pnpm, such as vp, available without direnv as well.
            shellHook = ''
              export PATH="$PWD/node_modules/.bin:$PATH"
            '';
          };
        };
    };
}
