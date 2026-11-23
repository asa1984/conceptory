import type { OxlintOverride } from "vite-plus/lint";

/** Settings for public libraries. Allows interface declarations so users can augment the types. */
export const libraryLint: Omit<OxlintOverride, "files"> = {
  rules: {
    "typescript/consistent-type-definitions": "off",
  },
};
