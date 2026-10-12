import { Suspense, use, ViewTransition } from "react";

import { getOverview } from "./data.ts";
import { Explorer } from "./explorer.tsx";
import { focusedDocument } from "./focus.ts";
import { useLive } from "./live-context.ts";
import { Page } from "./pages.tsx";
import { parseRoute } from "./route.ts";
import { useRouter } from "./router-context.ts";
import { SidePanel } from "./side-panel.tsx";
import { Callout, PageSkeleton } from "./ui.tsx";

const Status = () => {
  const { connection, reloadError } = useLive();
  if (connection === "closed") {
    return <Callout tone="danger">サーバーとの接続が切れました。再接続しています…</Callout>;
  }
  if (reloadError !== null) {
    return (
      <Callout tone="danger">
        読み込みに失敗したため、前回の内容を表示しています: {reloadError}
      </Callout>
    );
  }
  return null;
};

const Layout = () => {
  const { overviewVersion } = useLive();
  const overview = use(getOverview(overviewVersion));
  const { url, optimisticUrl, isPending } = useRouter();
  const route = parseRoute(url);
  const focused = focusedDocument({ route, overview });
  const optimisticFocused = focusedDocument({ route: parseRoute(optimisticUrl), overview });

  return (
    <div className="grid min-h-screen grid-cols-[16rem_minmax(0,1fr)_22rem] max-lg:grid-cols-1">
      <div
        aria-hidden="true"
        className={`fixed inset-x-0 top-0 z-10 h-0.5 bg-accent transition-opacity ${isPending ? "animate-pulse opacity-100" : "opacity-0"}`}
      />
      <Explorer overview={overview} selected={optimisticFocused} />
      <main className="min-w-0 px-10 py-8 max-lg:px-4">
        <Status />
        <Suspense fallback={<PageSkeleton />}>
          <ViewTransition key={`${route.view}:${focused ?? ""}`}>
            <Page route={route} focused={focused} />
          </ViewTransition>
        </Suspense>
      </main>
      <SidePanel overview={overview} focused={optimisticFocused} />
    </div>
  );
};

export const App = () => (
  <Suspense fallback={<PageSkeleton />}>
    <Layout />
  </Suspense>
);
