import { createContext, use } from "react";

export type Router = {
  /** URL of the screen being shown. */
  url: URL;
  /** URL of the destination during a navigation. Used to update the highlighted item immediately. */
  optimisticUrl: URL;
  isPending: boolean;
};

export const RouterContext = createContext<Router | null>(null);

export const useRouter = (): Router => {
  const router = use(RouterContext);
  if (router === null) {
    throw new Error("useRouter was called outside RouterProvider");
  }
  return router;
};
