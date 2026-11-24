# Builds conceptory from this repository with `pkgs`, which must have rust-overlay applied. The
# overlay and the flake's checks both call this, so they share the same derivations.
{ pkgs, crane }:
let
  inherit (pkgs) lib;
  root = ../.;
  version = (lib.importTOML (root + "/Cargo.toml")).workspace.package.version;

  # Use the same toolchain for the devShell and the package builds.
  rustToolchain = pkgs.rust-bin.fromRustupToolchainFile (root + "/rust-toolchain.toml");
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
  # conceptory_okf embeds the schemas with include_str! and the style guides in guides/ with
  # rust-embed. The clippy check reads clippy.toml.
  commonArgs = {
    src = lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        (root + "/Cargo.toml")
        (root + "/Cargo.lock")
        (root + "/clippy.toml")
        (root + "/crates")
        (root + "/guides")
        (root + "/schemas")
      ];
    };
    pname = "conceptory";
    inherit version;
    strictDeps = true;
  };
  # Build only the dependencies first and share them between the package and the checks.
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # The viewer SPA embedded in the conceptory CLI.
  viewer = pkgs.stdenv.mkDerivation (finalAttrs: {
    pname = "conceptory-viewer";
    inherit version;
    src = lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        (root + "/package.json")
        (root + "/pnpm-lock.yaml")
        (root + "/pnpm-workspace.yaml")
        (root + "/vite.config.ts")
        (root + "/packages/conceptory/package.json")
        (root + "/packages/conceptory/tsconfig")
        (root + "/packages/conceptory/src")
        (root + "/viewer/package.json")
        (root + "/viewer/index.html")
        (root + "/viewer/tsconfig.json")
        (root + "/viewer/tsconfig.app.json")
        (root + "/viewer/tsconfig.node.json")
        (root + "/viewer/vite.config.ts")
        (root + "/viewer/src")
      ];
    };
    pnpmDeps = pkgs.fetchPnpmDeps {
      inherit (finalAttrs) pname version src;
      inherit (pkgs) pnpm;
      fetcherVersion = 4;
      # pnpm leaves an empty v11/links in the store on darwin but not on Linux, which changes the hash.
      # Offline installs only need v11/files and the index.
      preFixup = ''
        rm -rf "$storePath/v11/links"
      '';
      hash = "sha256-RJPurbjAbHSiYwChKkrj3TO80AzYp8Id3lbIzS3XTMk=";
    };
    nativeBuildInputs = [
      pkgs.nodejs_26
      pkgs.pnpm
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
in
{
  inherit
    rustToolchain
    craneLib
    commonArgs
    cargoArtifacts
    viewer
    ;

  conceptory = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoExtraArgs = "--locked --package conceptory";
      # Tests run in the flake's checks.
      doCheck = false;
      env.CONCEPTORY_VIEWER_DIST = viewer;
      meta.mainProgram = "conceptory";
    }
  );

  # ast-grep rules distributed to downstream repositories, with one directory per language. Each
  # repository links to this package and points sgconfig.yaml at the directories for the languages
  # it uses.
  ast-grep-rules = pkgs.runCommand "conceptory-ast-grep-rules" { } ''
    cp -r ${
      lib.fileset.toSource {
        root = root + "/ast-grep/rules";
        fileset = root + "/ast-grep/rules";
      }
    } $out
  '';
}
