import { fmt } from "@conceptory/vite-plus-config/fmt";
import { lint, reactLint, testLint } from "@conceptory/vite-plus-config/lint";
import { test } from "@conceptory/vite-plus-config/test";
import { defineConfig } from "vite-plus";
import { configDefaults } from "vite-plus/test/config";

export default defineConfig({
  lint: {
    ...lint,
    overrides: [
      ...lint.overrides,
      { files: ["viewer/src/**"], ...reactLint },
      { files: ["**/*.{test,spec}.{ts,mts,cts,tsx}"], ...testLint },
    ],
  },
  fmt: {
    ...fmt,
    // taplo formats TOML.
    ignorePatterns: ["**/*.toml"],
  },
  test: {
    ...test,
    // .direnv, where direnv unpacks the flake inputs, also contains tests from other projects.
    exclude: [...configDefaults.exclude, "**/.direnv/**"],
  },
  staged: {
    "*": "vp check --fix --no-error-on-unmatched-pattern",
    "*.{js,mjs,cjs,ts,mts,cts,jsx,tsx}": "ast-grep scan",
    "*.rs": "rustfmt",
    "*.toml": "taplo fmt",
    "*.nix": "nixfmt",
  },
  run: {
    tasks: {
      // Runs every check in the repository in addition to formatting, linting, and type checking.
      check: {
        command: "vp check",
        dependsOn: [
          "check:actions",
          "check:ast-grep",
          "check:catalog",
          "check:skills",
          "check:files",
          "check:renovate",
          "check:rust",
          "check:toml",
          "okf:check",
          "check:test",
          "textlint",
        ],
        cache: false,
      },
      "check:actions": {
        command: ["actionlint", "zizmor --offline .github/workflows"],
        cache: false,
      },
      // Checks TypeScript rules that oxlint cannot express, and tests the rules themselves.
      "check:ast-grep": {
        command: ["ast-grep scan", "ast-grep test --config ast-grep/sgconfig.yaml"],
        cache: false,
      },
      "check:catalog": {
        command: "node scripts/check-catalog.ts",
        cache: false,
      },
      // Ensures the symlinks in skills' references/ point to existing files (ADR 0004 "Define style guides for skills").
      "check:skills": {
        command: 'test -z "$(find -L skills -type l)"',
        cache: false,
      },
      // Checks that YAML files use the .yaml extension (ADR 0007 "Use the .yaml extension").
      "check:files": {
        command: "cargo run --quiet --package conceptory -- files check",
        cache: false,
      },
      // Checks the distributed Renovate presets and conceptory's own Renovate config.
      "check:renovate": {
        // Run through a shell to expand the glob.
        command:
          "sh -c 'renovate-config-validator --strict --no-global renovate/*.json5 renovate.json5'",
        cache: false,
      },
      "check:rust": {
        command: ["cargo fmt --check", "cargo clippy --all-targets -- -D warnings", "cargo test"],
        cache: false,
      },
      "check:test": {
        command: "vp test run",
        cache: false,
      },
      "check:toml": {
        command: "taplo fmt --check",
        cache: false,
      },
      "okf:check": {
        command: "cargo run --quiet --package conceptory -- okf check docs guides",
        cache: false,
      },
      // Regenerates each directory's index.md from frontmatter.
      "okf:index": {
        command: "cargo run --quiet --package conceptory -- okf index docs guides",
        cache: false,
      },
      // Generates the textlint dictionaries from the terms and vocabulary in the frontmatter of docs/ and guides/.
      "okf:dictionaries": {
        command: [
          "cargo run --quiet --package conceptory -- okf terms docs guides --prh .textlint/prh.yaml",
          "cargo run --quiet --package conceptory -- okf vocabulary docs guides --out .textlint/vocabulary.json",
        ],
        cache: false,
      },
      // Opens docs/ and guides/ in the viewer and keeps it in sync with file changes.
      "okf:view": {
        command: "cargo run --quiet --package conceptory -- okf view docs guides",
        cache: false,
      },
      textlint: {
        command: "textlint docs guides skills",
        dependsOn: ["okf:dictionaries"],
        cache: false,
      },
    },
  },
});
