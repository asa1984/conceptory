/**
 * The textlint preset that bundles the rule sets used to check Japanese prose (ADR 0005 "Check
 * Japanese in two stages").
 *
 * Rule IDs are prefixed with the name of the rule set they come from, such as
 * `ja-technical-writing/max-ten`. The dictionaries are read from `.textlint/`, where
 * `conceptory okf terms` and `conceptory okf vocabulary` write them.
 */

import aiWriting from "@textlint-ja/textlint-rule-preset-ai-writing";
import aiWordsJa from "textlint-rule-preset-ai-words-ja";
import jaTechnicalWriting from "textlint-rule-preset-ja-technical-writing";
import prh from "textlint-rule-prh";

type Preset = {
  rules: Record<string, unknown>;
  rulesConfig: Record<string, unknown>;
};

/** Overrides of the rule sets' defaults, keyed by the rule IDs within each rule set. */
type Overrides = Record<string, unknown>;

const rules: Record<string, unknown> = {};
const rulesConfig: Record<string, unknown> = {};

const include = ({
  prefix,
  preset,
  overrides = {},
}: {
  prefix: string;
  preset: Preset;
  overrides?: Overrides;
}): void => {
  for (const [id, rule] of Object.entries(preset.rules)) {
    rules[`${prefix}/${id}`] = rule;
    rulesConfig[`${prefix}/${id}`] = overrides[id] ?? preset.rulesConfig[id] ?? true;
  }
};

include({
  prefix: "ja-technical-writing",
  preset: jaTechnicalWriting,
  overrides: {
    // Splitting sentences to fit a character count fragments the prose, and inline code and ADR
    // titles inflate the count, so sentence length is left to review.
    "sentence-length": false,
    // Documents use the plain (dearu) style.
    "no-mix-dearu-desumasu": {
      preferInHeader: "",
      preferInBody: "である",
      preferInList: "である",
      strict: true,
    },
  },
});
include({ prefix: "ai-writing", preset: aiWriting.default });
include({
  prefix: "ai-words-ja",
  preset: aiWordsJa.default,
  overrides: {
    "no-ai-words": {
      // Dictionary-based rules can also flag correct usage, so report them as warnings.
      severity: "warning",
      dictionaryPath: "./.textlint/vocabulary.json",
    },
  },
});
rules["prh"] = prh.default;
rulesConfig["prh"] = { rulePaths: ["./.textlint/prh.yaml"] };

const preset: Preset = { rules, rulesConfig };

// textlint loads a preset from the default export.
// oxlint-disable-next-line import/no-default-export
export default preset;
