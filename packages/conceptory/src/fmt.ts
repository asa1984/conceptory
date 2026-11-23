/** The oxfmt baseline. */

import type { Oxfmtrc } from "vite-plus/fmt";

export const fmt: Oxfmtrc = {
  // Make import order deterministic so diffs don't include reorder-only changes.
  sortImports: true,
  sortPackageJson: true,
};
