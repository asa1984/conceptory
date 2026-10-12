import { createContext, use } from "react";

export type Connection = "connecting" | "open" | "closed";

export type Live = {
  connection: Connection;
  /** Error from the last failed reload. Cleared by the next successful reload. */
  reloadError: string | null;
  overviewVersion: string;
  documentVersion: (id: string) => string;
};

export const LiveContext = createContext<Live | null>(null);

export const useLive = (): Live => {
  const live = use(LiveContext);
  if (live === null) {
    throw new Error("useLive was called outside LiveProvider");
  }
  return live;
};
