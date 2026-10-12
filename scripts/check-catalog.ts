// Checks that every dependency spec in the workspace's package.json files is catalog: or
// workspace:. pnpm's catalogMode: strict only restricts versions added with pnpm add, so specs
// written directly in package.json are caught here. peerDependencies are excluded because
// distributed packages specify them as ranges.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, relative } from "node:path";

const fields = ["dependencies", "devDependencies", "optionalDependencies"] as const;
const allowedProtocols = ["catalog:", "workspace:"];

type Project = { path: string };
type Manifest = Partial<Record<(typeof fields)[number], Record<string, string>>>;

/**
 * Types the JSON from pnpm and package.json. pnpm guarantees both formats, so the contents are
 * trusted without validation.
 */
/* oxlint-disable typescript/no-unnecessary-type-parameters, typescript/no-unsafe-type-assertion */
const decodeJson = <T>(text: string): T => JSON.parse(text) as T;
/* oxlint-enable typescript/no-unnecessary-type-parameters, typescript/no-unsafe-type-assertion */

const projects = decodeJson<Project[]>(
  execFileSync("pnpm", ["ls", "--recursive", "--depth", "-1", "--json"], { encoding: "utf8" }),
);

const violations: string[] = [];
for (const project of projects) {
  const manifestPath = join(project.path, "package.json");
  const manifest = decodeJson<Manifest>(readFileSync(manifestPath, "utf8"));
  for (const field of fields) {
    for (const [name, spec] of Object.entries(manifest[field] ?? {})) {
      if (!allowedProtocols.some((protocol) => spec.startsWith(protocol))) {
        violations.push(`${relative(process.cwd(), manifestPath)}: ${field}.${name} = "${spec}"`);
      }
    }
  }
}

if (violations.length > 0) {
  console.error("catalog: または workspace: を使っていない依存があります。");
  for (const violation of violations) {
    console.error(`  ${violation}`);
  }
  process.exitCode = 1;
}
