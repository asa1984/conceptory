// Shows the directory structure of an OKF bundle as a tree. A directory's index.md appears as a
// link on the directory's name.

import { useDeferredValue, useState } from "react";

import type { DocumentSummary, Overview } from "./data.ts";
import { href } from "./route.ts";

type Folder = {
  /** Display name. Levels with a single child directory are collapsed into `a/b`. */
  name: string;
  index: DocumentSummary | null;
  folders: Folder[];
  documents: DocumentSummary[];
};

type MutableFolder = {
  name: string;
  index: DocumentSummary | null;
  folders: Map<string, MutableFolder>;
  documents: DocumentSummary[];
};

const emptyFolder = (name: string): MutableFolder => ({
  name,
  index: null,
  folders: new Map(),
  documents: [],
});

const insert = ({ root, document }: { root: MutableFolder; document: DocumentSummary }): void => {
  const segments = document.id.split("/").filter((segment) => segment !== "");
  const fileName = segments.pop() ?? document.id;
  let folder = root;
  for (const segment of segments) {
    const child = folder.folders.get(segment) ?? emptyFolder(segment);
    folder.folders.set(segment, child);
    folder = child;
  }
  if (fileName === "index.md") {
    folder.index = document;
  } else {
    folder.documents.push(document);
  }
};

/** Merges levels that have no files and a single child directory into one level. */
const compact = (folder: MutableFolder): Folder => {
  let current = folder;
  let { name } = folder;
  while (current.index === null && current.documents.length === 0 && current.folders.size === 1) {
    const [child] = current.folders.values();
    if (child === undefined) {
      break;
    }
    name = name === "" ? child.name : `${name}/${child.name}`;
    current = child;
  }
  return {
    name,
    index: current.index,
    folders: [...current.folders.values()]
      .map((child) => compact(child))
      .toSorted((a, b) => a.name.localeCompare(b.name)),
    documents: current.documents.toSorted((a, b) => a.id.localeCompare(b.id)),
  };
};

const buildTree = (documents: DocumentSummary[]): Folder => {
  const root = emptyFolder("");
  for (const document of documents) {
    insert({ root, document });
  }
  return compact(root);
};

const matches = ({ document, words }: { document: DocumentSummary; words: string[] }): boolean => {
  const text = [document.title, document.id, ...document.tags].join(" ").toLowerCase();
  return words.every((word) => text.includes(word));
};

const DocumentLink = ({
  document,
  label,
  selected,
}: {
  document: DocumentSummary;
  label: string;
  selected: boolean;
}) => (
  <a
    href={href({ view: "document", id: document.id })}
    aria-current={selected ? "page" : undefined}
    title={document.id}
    className={`block truncate rounded px-2 py-0.5 hover:bg-canvas-subtle aria-[current=page]:bg-accent-subtle aria-[current=page]:font-semibold ${document.deprecated_by === null ? "" : "text-fg-muted line-through"}`}
  >
    {label}
  </a>
);

const FolderView = ({ folder, selected }: { folder: Folder; selected: string | null }) => (
  <li>
    {folder.index === null ? (
      <span className="block px-2 py-0.5 text-fg-muted">{folder.name}</span>
    ) : (
      <DocumentLink
        document={folder.index}
        label={folder.index.title}
        selected={folder.index.id === selected}
      />
    )}
    <ul className="ml-3 border-l border-line pl-1">
      {folder.folders.map((child) => (
        <FolderView key={child.name} folder={child} selected={selected} />
      ))}
      {folder.documents.map((document) => (
        <li key={document.id}>
          <DocumentLink
            document={document}
            label={document.title}
            selected={document.id === selected}
          />
        </li>
      ))}
    </ul>
  </li>
);

const SearchResults = ({
  documents,
  selected,
}: {
  documents: DocumentSummary[];
  selected: string | null;
}) =>
  documents.length === 0 ? (
    <p className="px-2 text-fg-muted">該当する文書はありません。</p>
  ) : (
    <ul>
      {documents.map((document) => (
        <li key={document.id}>
          <DocumentLink
            document={document}
            label={document.title}
            selected={document.id === selected}
          />
        </li>
      ))}
    </ul>
  );

const Footer = ({ problems }: { problems: number }) => (
  <div className="border-t border-line p-3">
    <a href={href({ view: "terms" })} className="block rounded px-2 py-0.5 hover:bg-canvas">
      用語集
    </a>
    <a
      href={href({ view: "problems" })}
      className="flex items-center justify-between rounded px-2 py-0.5 hover:bg-canvas"
    >
      問題
      {problems > 0 && (
        <span className="rounded-full bg-danger px-2 text-xs text-canvas">{problems}</span>
      )}
    </a>
  </div>
);

const Tree = ({
  documents,
  selected,
}: {
  documents: DocumentSummary[];
  selected: string | null;
}) => {
  const tree = buildTree(documents);
  return (
    <ul>
      {tree.folders.map((folder) => (
        <FolderView key={folder.name} folder={folder} selected={selected} />
      ))}
      {tree.documents.map((document) => (
        <li key={document.id}>
          <DocumentLink
            document={document}
            label={document.title}
            selected={document.id === selected}
          />
        </li>
      ))}
    </ul>
  );
};

type Props = {
  overview: Overview;
  /** The document to highlight. During a navigation, this is already the destination. */
  selected: string | null;
};

export const Explorer = ({ overview, selected }: Props) => {
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query);
  const words = deferredQuery
    .toLowerCase()
    .split(/\s+/u)
    .filter((word) => word !== "");

  return (
    <nav
      aria-label="文書"
      className="sticky top-0 flex h-screen flex-col border-r border-line bg-canvas-subtle text-sm max-lg:static max-lg:h-auto max-lg:max-h-[40vh]"
    >
      <div className="p-3">
        <a href={href({ view: "home" })} className="mb-3 block px-2 font-semibold">
          conceptory
        </a>
        <input
          type="search"
          placeholder="検索"
          aria-label="文書を検索"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
          className="w-full rounded-md border border-line bg-canvas px-2 py-1"
        />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        {words.length > 0 ? (
          <SearchResults
            documents={overview.documents.filter((document) => matches({ document, words }))}
            selected={selected}
          />
        ) : (
          <Tree documents={overview.documents} selected={selected} />
        )}
      </div>
      <Footer problems={overview.diagnostics.length} />
    </nav>
  );
};
