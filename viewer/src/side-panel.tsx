import { useState } from "react";

import { titleLookup } from "./data.ts";
import type { Overview } from "./data.ts";
import { Graph } from "./graph.tsx";
import type { GraphMode } from "./graph.tsx";
import { href } from "./route.ts";
import { SectionHeading } from "./ui.tsx";

const modes: { mode: GraphMode; label: string }[] = [
  { mode: "local", label: "ローカル" },
  { mode: "global", label: "全体" },
];

const DocumentLinks = ({ ids, titles }: { ids: string[]; titles: (id: string) => string }) =>
  ids.length === 0 ? (
    <p className="text-fg-muted">なし</p>
  ) : (
    <ul>
      {ids.map((id) => (
        <li key={id}>
          <a
            href={href({ view: "document", id })}
            className="block truncate rounded px-1 hover:bg-canvas-subtle"
          >
            {titles(id)}
          </a>
        </li>
      ))}
    </ul>
  );

/** The graph and a switch for its scope. */
const GraphSection = ({ overview, focused }: Props) => {
  const [mode, setMode] = useState<GraphMode>("local");
  return (
    <section>
      <div className="mb-1 flex items-center justify-between">
        <SectionHeading>グラフ</SectionHeading>
        <fieldset className="flex rounded-md border border-line text-xs">
          <legend className="sr-only">グラフの範囲</legend>
          {modes.map((item) => (
            <button
              key={item.mode}
              type="button"
              aria-pressed={mode === item.mode}
              onClick={() => {
                setMode(item.mode);
              }}
              className="px-2 py-0.5 aria-pressed:bg-accent-subtle aria-pressed:text-accent"
            >
              {item.label}
            </button>
          ))}
        </fieldset>
      </div>
      <Graph overview={overview} focused={focused} mode={mode} />
    </section>
  );
};

/** Documents linked to or from the current document, and the terms it defines. */
const Connections = ({ overview, focused }: { overview: Overview; focused: string }) => {
  const titles = titleLookup(overview);
  const links = overview.edges.filter((edge) => edge.kind === "link");
  const backlinks = links.filter((edge) => edge.target === focused).map((edge) => edge.source);
  const outgoing = links.filter((edge) => edge.source === focused).map((edge) => edge.target);
  const terms = overview.terms.filter((term) => term.document === focused);
  return (
    <>
      <section>
        <SectionHeading>バックリンク</SectionHeading>
        <DocumentLinks ids={backlinks} titles={titles} />
      </section>
      <section>
        <SectionHeading>リンク先</SectionHeading>
        <DocumentLinks ids={outgoing} titles={titles} />
      </section>
      {terms.length > 0 && (
        <section>
          <SectionHeading>定義している用語</SectionHeading>
          <ul>
            {terms.map((term) => (
              <li key={term.name} className="px-1">
                {term.name}
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
};

type Props = {
  overview: Overview;
  /** The current document, or the destination during a navigation. */
  focused: string | null;
};

export const SidePanel = ({ overview, focused }: Props) => (
  <aside className="sticky top-0 h-screen space-y-5 overflow-y-auto border-l border-line p-3 text-sm max-lg:static max-lg:h-auto">
    <GraphSection overview={overview} focused={focused} />
    {focused !== null && <Connections overview={overview} focused={focused} />}
  </aside>
);
