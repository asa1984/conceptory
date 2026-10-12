// An ast-grep rule can't target both TypeScript and TSX, so each rule is written once per
// language. This test checks that both versions match except for their ID and language.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test } from "vite-plus/test";

const directory = join(import.meta.dirname, "rules/typescript");

const files = readdirSync(directory).filter((file) => file.endsWith(".yaml"));

/** Rewrites a TSX rule as its TypeScript counterpart. */
const asTypeScript = (document: string): string =>
  document
    .replace(/^id: tsx\//mu, "id: typescript/")
    .replace(/^language: Tsx$/mu, "language: TypeScript");

/** Splits a rule file into YAML documents, trimming surrounding whitespace. */
const documentsOf = (file: string): string[] =>
  readFileSync(join(directory, file), "utf8")
    .split(/^---$/mu)
    .map((document) => document.trim());

test.each(files)("%s matches between TypeScript and TSX", (file) => {
  const documents = documentsOf(file);
  expect(documents).toHaveLength(2);
  const [typescript = "", tsx = ""] = documents;
  expect(typescript).toMatch(/^id: typescript\//u);
  expect(asTypeScript(tsx)).toBe(typescript);
});
