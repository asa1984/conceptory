import { DocumentPage } from "./document-page.tsx";
import { ProblemsPage } from "./problems-page.tsx";
import type { Route } from "./route.ts";
import { TermsPage } from "./terms-page.tsx";

export const Page = ({ route, focused }: { route: Route; focused: string | null }) => {
  switch (route.view) {
    case "terms": {
      return <TermsPage />;
    }
    case "problems": {
      return <ProblemsPage />;
    }
    case "home":
    case "document": {
      return focused === null ? (
        <p className="text-fg-muted">文書がありません。</p>
      ) : (
        <DocumentPage id={focused} />
      );
    }
  }
};
