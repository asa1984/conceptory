import { use } from "react";

import { getOverview } from "./data.ts";
import { useLive } from "./live-context.ts";
import { ProblemList } from "./problem-list.tsx";

export const ProblemsPage = () => {
  const { overviewVersion } = useLive();
  const { documents, diagnostics } = use(getOverview(overviewVersion));
  return (
    <section className="mx-auto max-w-4xl">
      <h1 className="mb-4 text-2xl font-semibold">問題</h1>
      {diagnostics.length === 0 ? (
        <p className="text-fg-muted">問題はありません。</p>
      ) : (
        <ProblemList
          problems={diagnostics}
          linkable={new Set(documents.map((document) => document.id))}
        />
      )}
    </section>
  );
};
