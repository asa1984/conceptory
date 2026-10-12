import { use } from "react";

import { getOverview, titleLookup } from "./data.ts";
import { useLive } from "./live-context.ts";
import { href } from "./route.ts";

export const TermsPage = () => {
  const { overviewVersion } = useLive();
  const overview = use(getOverview(overviewVersion));
  const titles = titleLookup(overview);
  const terms = overview.terms.toSorted((a, b) => a.name.localeCompare(b.name));
  return (
    <section className="mx-auto max-w-4xl">
      <h1 className="mb-4 text-2xl font-semibold">用語集</h1>
      {terms.length === 0 ? (
        <p className="text-fg-muted">用語はまだ定義されていません。</p>
      ) : (
        <table className="w-full border-collapse text-sm">
          <thead>
            <tr className="border-b border-line text-left text-fg-muted">
              <th scope="col" className="py-2 pr-4 font-semibold">
                用語
              </th>
              <th scope="col" className="py-2 pr-4 font-semibold">
                避ける表記
              </th>
              <th scope="col" className="py-2 font-semibold">
                定義している文書
              </th>
            </tr>
          </thead>
          <tbody>
            {terms.map((term) => (
              <tr key={term.name} className="border-b border-line">
                <td className="py-2 pr-4">{term.name}</td>
                <td className="py-2 pr-4 text-fg-muted">{term.avoid.join("、")}</td>
                <td className="py-2">
                  <a
                    className="text-accent hover:underline"
                    href={href({ view: "document", id: term.document })}
                  >
                    {titles(term.document)}
                  </a>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
};
