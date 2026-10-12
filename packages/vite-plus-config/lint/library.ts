import type { OxlintOverride } from "vite-plus/lint";

/** Settings for public libraries. Allows interface declarations so users can augment the types. */
export const libraryLint = {
  rules: {
    "typescript/consistent-type-definitions": "off",
  },
} satisfies Omit<OxlintOverride, "files">;
