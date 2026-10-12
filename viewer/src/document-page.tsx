import { use } from "react";

import { getDocument, getOverview, titleLookup } from "./data.ts";
import type { DocumentDetail, Overview } from "./data.ts";
import { useLive } from "./live-context.ts";
import { MarkdownBody } from "./markdown-body.tsx";
import { ProblemList } from "./problem-list.tsx";
import { href } from "./route.ts";
import { Callout } from "./ui.tsx";

const Properties = ({ document }: { document: DocumentDetail }) => (
  <div className="mb-6 space-y-1 text-sm">
    <p className="font-mono text-xs break-all text-fg-muted">{document.id}</p>
    {(document.type !== null || document.tags.length > 0) && (
      <p className="flex flex-wrap gap-1.5">
        {document.type !== null && (
          <span className="rounded-full border border-accent px-2 text-accent">
            {document.type}
          </span>
        )}
        {document.tags.map((tag) => (
          <span key={tag} className="rounded-full border border-line px-2 text-fg-muted">
            #{tag}
          </span>
        ))}
      </p>
    )}
  </div>
);

const FoundDocument = ({
  document,
  overview,
}: {
  document: DocumentDetail;
  overview: Overview;
}) => {
  const titles = titleLookup(overview);
  const problems = overview.diagnostics.filter((diagnostic) => diagnostic.path === document.id);
  return (
    <article className="mx-auto max-w-4xl">
      <Properties document={document} />
      {document.deprecated_by !== null && (
        <Callout tone="attention">
          この文書は{" "}
          <a
            className="text-accent underline"
            href={href({ view: "document", id: document.deprecated_by })}
          >
            {titles(document.deprecated_by)}
          </a>{" "}
          によって廃止されました。
        </Callout>
      )}
      {problems.length > 0 && (
        <Callout tone="danger">
          <ProblemList problems={problems} />
        </Callout>
      )}
      <MarkdownBody key={document.id} html={document.html} />
    </article>
  );
};

export const DocumentPage = ({ id }: { id: string }) => {
  const { overviewVersion, documentVersion } = useLive();
  const overview = use(getOverview(overviewVersion));
  const result = use(getDocument({ id, version: documentVersion(id) }));
  switch (result.status) {
    case "found": {
      return <FoundDocument document={result.document} overview={overview} />;
    }
    case "missing": {
      return (
        <p className="text-fg-muted">
          <code>{id}</code> は見つかりません。削除されたか、名前が変わった可能性があります。
        </p>
      );
    }
    case "failed": {
      return <Callout tone="danger">文書を取得できませんでした: {result.message}</Callout>;
    }
  }
};
