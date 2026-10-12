// Types of the data `conceptory okf serve` returns, and a fetch cache for reading it with Suspense.
//
// `use` needs the same Promise object for the same data. Cache entries carry a version, and a
// change notification from the server bumps the version so that a new Promise is created.

export type Kind = "concept" | "index" | "log";

export type DocumentSummary = {
  id: string;
  title: string;
  type: string | null;
  kind: Kind;
  description: string | null;
  tags: string[];
  deprecated_by: string | null;
  hash: string;
};

export type DocumentDetail = DocumentSummary & {
  frontmatter: unknown;
  html: string;
  links: string[];
  backlinks: string[];
  terms: string[];
};

export type Edge = {
  source: string;
  target: string;
  kind: "link" | "deprecated-by";
};

export type Term = {
  name: string;
  avoid: string[];
  document: string;
};

export type Diagnostic = {
  path: string;
  line: number;
  rule: string;
  message: string;
};

export type Overview = {
  documents: DocumentSummary[];
  edges: Edge[];
  terms: Term[];
  diagnostics: Diagnostic[];
};

type OverviewResponse = {
  serverId: string;
  revision: number;
  overview: Overview;
};

export type Hello = {
  serverId: string;
  revision: number;
};

export type Update =
  | { type: "changed"; revision: number; changed: string[]; removed: string[]; overview: boolean }
  | { type: "error"; message: string };

/** Result of fetching a document. Failures are values too, so rendering doesn't handle exceptions. */
export type DocumentResult =
  | { status: "found"; document: DocumentDetail }
  | { status: "missing" }
  | { status: "failed"; message: string };

/**
 * Types the JSON the server returns. The server lives in the same repository and its types change
 * together with its implementation, so the contents are trusted without validation.
 */
/* oxlint-disable typescript/no-unnecessary-type-parameters, typescript/no-unsafe-type-assertion */
export const decodeJson = <T>(text: string): T => JSON.parse(text) as T;
/* oxlint-enable typescript/no-unnecessary-type-parameters, typescript/no-unsafe-type-assertion */

const wait = async (milliseconds: number): Promise<void> => {
  await new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
};

/**
 * Fetches the overview, retrying until it succeeds because no screen can render without it.
 * Meanwhile the Suspense fallback is shown, and the connection state reports a lost connection.
 */
const fetchOverview = async (): Promise<Overview> => {
  for (;;) {
    try {
      // Attempts run one at a time, so await inside the loop.
      // oxlint-disable-next-line no-await-in-loop
      const response = await fetch("/api/overview", { cache: "no-store" });
      if (response.ok) {
        // oxlint-disable-next-line no-await-in-loop
        return decodeJson<OverviewResponse>(await response.text()).overview;
      }
    } catch {
      // If the server is unreachable, wait a moment before retrying.
    }
    // oxlint-disable-next-line no-await-in-loop
    await wait(1000);
  }
};

const fetchDocument = async (id: string): Promise<DocumentResult> => {
  try {
    const response = await fetch(`/api/document?id=${encodeURIComponent(id)}`, {
      cache: "no-store",
    });
    if (response.status === 404) {
      return { status: "missing" };
    }
    if (!response.ok) {
      return { status: "failed", message: response.status.toString() };
    }
    return { status: "found", document: decodeJson<DocumentDetail>(await response.text()) };
  } catch (error) {
    return { status: "failed", message: error instanceof Error ? error.message : String(error) };
  }
};

type Entry<T> = {
  version: string;
  promise: Promise<T>;
};

const overviews = new Map<string, Entry<Overview>>();
const documents = new Map<string, Entry<DocumentResult>>();

/** Keeps only the latest version's Promise per key and refetches when the version changes. */
const cached = <T>({
  store,
  key,
  version,
  load,
}: {
  store: Map<string, Entry<T>>;
  key: string;
  version: string;
  load: () => Promise<T>;
}): Promise<T> => {
  const entry = store.get(key);
  if (entry?.version === version) {
    return entry.promise;
  }
  const promise = load();
  store.set(key, { version, promise });
  return promise;
};

export const getOverview = (version: string): Promise<Overview> =>
  cached({ store: overviews, key: "", version, load: fetchOverview });

export const getDocument = ({
  id,
  version,
}: {
  id: string;
  version: string;
}): Promise<DocumentResult> =>
  cached({ store: documents, key: id, version, load: () => fetchDocument(id) });

/** Returns a function that looks up a document's title in the overview by its ID. */
export const titleLookup = (overview: Overview): ((id: string) => string) => {
  const titles = new Map(overview.documents.map((document) => [document.id, document.title]));
  return (id) => titles.get(id) ?? id;
};
