// Installs the tarball of @asa1984/conceptory into a temporary project, as a downstream repository
// would, and checks that the tsconfig, the Vite+ configs, and the textlint preset load from
// node_modules (ADR 0019 "Distribute artifacts via npm, Nix flake, and the conceptory CLI").

import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const packageDir = join(import.meta.dirname, "../packages/conceptory");
const project = mkdtempSync(join(tmpdir(), "conceptory-package-"));

/** Returns the version of a package installed in the workspace, so the project uses the same one. */
const installedVersion = (name: string): string => {
  const manifest = readFileSync(
    join(import.meta.dirname, "../node_modules", name, "package.json"),
    {
      encoding: "utf8",
    },
  );
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- package.json has a version.
  return (JSON.parse(manifest) as { version: string }).version;
};

const run = ({ command, args }: { command: string; args: string[] }): string =>
  execFileSync(command, args, { cwd: project, encoding: "utf8", stdio: "pipe" });

const files: Record<string, string> = {
  "package.json": `${JSON.stringify({ private: true, type: "module" }, undefined, 2)}\n`,
  "tsconfig.json": `${JSON.stringify(
    {
      extends: [
        "@asa1984/conceptory/tsconfig/base.json",
        "@asa1984/conceptory/tsconfig/bundler.json",
        "@asa1984/conceptory/tsconfig/dom.json",
        "@asa1984/conceptory/tsconfig/react.json",
        "@asa1984/conceptory/tsconfig/library.json",
      ],
      compilerOptions: { types: ["node"] },
      include: ["vite.config.ts"],
    },
    undefined,
    2,
  )}\n`,
  "vite.config.ts": [
    'import { fmt } from "@asa1984/conceptory/fmt";',
    'import { lint } from "@asa1984/conceptory/lint";',
    'import { test } from "@asa1984/conceptory/test";',
    'import { defineConfig } from "vite-plus";',
    "",
    "export default defineConfig({ lint, fmt, test });",
    "",
  ].join("\n"),
  ".textlintrc.yaml": [
    "filters:",
    "  comments: true",
    "rules:",
    '  "@asa1984/conceptory/preset-textlint": true',
    "",
  ].join("\n"),
  ".textlint/vocabulary.json": '{ "entries": [] }\n',
  ".textlint/prh.yaml": "version: 1\nrules: []\n",
  // Repeats the particle は, which ja-technical-writing/no-doubled-joshi reports.
  "doc.md": "# 文書\n\n私は彼は好きだ。\n",
};

try {
  execFileSync("vp", ["pack"], { cwd: packageDir, stdio: "inherit" });
  execFileSync("pnpm", ["pack", "--pack-destination", project], { cwd: packageDir, stdio: "pipe" });
  const tarball = readdirSync(project).find((name) => name.endsWith(".tgz"));
  if (tarball === undefined) {
    throw new Error("pnpm pack did not write a tarball");
  }

  mkdirSync(join(project, ".textlint"));
  for (const [path, content] of Object.entries(files)) {
    writeFileSync(join(project, path), content);
  }
  run({
    command: "pnpm",
    args: [
      "add",
      "--save-dev",
      `./${tarball}`,
      ...["@types/node", "textlint", "textlint-filter-rule-comments", "vite-plus"].map(
        (name) => `${name}@${installedVersion(name)}`,
      ),
    ],
  });

  // The lint config enables type checking, which resolves the tsconfig presets and the types of the
  // configs. Linting itself loads the configs. The file is passed explicitly because the project has
  // no .gitignore, so linting the directory would also lint node_modules.
  run({ command: "pnpm", args: ["exec", "vp", "lint", "vite.config.ts"] });

  // textlint exits with a failure when it reports problems, so read its output regardless.
  const report = spawnSync("pnpm", ["exec", "textlint", "doc.md"], {
    cwd: project,
    encoding: "utf8",
  }).stdout;
  if (
    !report.includes("@asa1984/conceptory/preset-textlint/ja-technical-writing/no-doubled-joshi")
  ) {
    throw new Error(`textlint did not report with the preset's rules:\n${report}`);
  }
} finally {
  rmSync(project, { recursive: true, force: true });
}
