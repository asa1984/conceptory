// Records the rules in the wholly enabled categories. If an oxlint update adds or removes rules in
// those categories, this test fails; review the diff, then update the snapshot.

import { execFileSync } from "node:child_process";

import { expect, test } from "vite-plus/test";

import { basePlugins, enabledCategories, reactLint, testLint } from "./lint.ts";

type Rule = {
  scope: string;
  value: string;
  category: string;
};

/** Types the output of oxlint --rules. oxlint guarantees its format. */
const listRules = (): Rule[] => {
  const output = execFileSync("vp", ["lint", "--rules", "--format", "json"], { encoding: "utf8" });
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return JSON.parse(output) as Rule[];
};

/** Converts a plugin name to the scope notation oxlint --rules prints. */
const scopeOf = (plugin: string): string => plugin.replaceAll("-", "_");

const enabledPlugins = new Set(
  [...basePlugins, ...reactLint.plugins, ...testLint.plugins].map((plugin) => scopeOf(plugin)),
);

const isEnabled = ({ rule, category }: { rule: Rule; category: string }): boolean =>
  rule.category === category && enabledPlugins.has(rule.scope);

const rulesByCategory = (rules: Rule[]): Record<string, string[]> =>
  Object.fromEntries(
    enabledCategories.map((category) => [
      category,
      rules
        .filter((rule) => isEnabled({ rule, category }))
        .map((rule) => `${rule.scope}/${rule.value}`)
        .toSorted(),
    ]),
  );

test("rules in wholly enabled categories", () => {
  expect(rulesByCategory(listRules())).toMatchSnapshot();
});
