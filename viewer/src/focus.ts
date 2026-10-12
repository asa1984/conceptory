import type { DocumentSummary, Overview } from "./data.ts";
import type { Route } from "./route.ts";

const depth = (id: string): number => id.split("/").length;

/** The shallowest index.md is the document shown first. */
const homeDocument = (documents: DocumentSummary[]): DocumentSummary | undefined =>
  documents
    .filter((document) => document.kind === "index")
    .toSorted((a, b) => depth(a.id) - depth(b.id) || a.id.localeCompare(b.id))[0];

/** The current document. On the top page, this is the document shown first. */
export const focusedDocument = ({
  route,
  overview,
}: {
  route: Route;
  overview: Overview;
}): string | null => {
  switch (route.view) {
    case "document": {
      return route.id;
    }
    case "home": {
      return homeDocument(overview.documents)?.id ?? null;
    }
    case "terms":
    case "problems": {
      return null;
    }
  }
};
