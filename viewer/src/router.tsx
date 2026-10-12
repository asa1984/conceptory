// Intercepts same-document navigations with the Navigation API and switches screens in a React
// transition.
//
// When a navigation is intercepted, the browser defers scroll and focus restoration until the
// handler's Promise resolves. The Promise resolves after the transition commits to the DOM, so
// restoration applies to the new screen. During a navigation, the optimistic URL from useOptimistic
// highlights the selected item immediately.

import {
  addTransitionType,
  useEffect,
  useLayoutEffect,
  useOptimistic,
  useState,
  useTransition,
} from "react";
import type { ReactNode } from "react";

import { scrollToHash } from "./route.ts";
import { RouterContext } from "./router-context.ts";

type State = {
  url: URL;
  /** Called after the render commits to the DOM to tell the browser the navigation has finished. */
  commit: () => void;
};

/** Intercepts navigations to viewer screens and updates the URL in a transition. */
const useNavigationTransitions = () => {
  const [state, setState] = useState<State>(() => ({
    url: new URL(globalThis.location.href),
    commit: () => {},
  }));
  const [optimisticUrl, setOptimisticUrl] = useOptimistic(state.url);
  const [isPending, startTransition] = useTransition();

  useEffect(() => {
    const onNavigate = (event: NavigateEvent): void => {
      const url = new URL(event.destination.url);
      // Handle only viewer screens and leave navigations such as /files to the browser.
      if (!event.canIntercept || event.downloadRequest !== null || url.pathname !== "/") {
        return;
      }
      // Body ids are prefixed with user-content-, so scroll to the # target manually.
      event.intercept({
        scroll: url.hash === "" ? "after-transition" : "manual",
        handler: async () => {
          if (!event.hashChange) {
            await new Promise<void>((resolve) => {
              startTransition(() => {
                addTransitionType(`navigation-${event.navigationType}`);
                setOptimisticUrl(url);
                setState({ url, commit: resolve });
              });
            });
          }
          scrollToHash(url.hash);
        },
      });
    };
    navigation.addEventListener("navigate", onNavigate);
    return () => {
      navigation.removeEventListener("navigate", onNavigate);
    };
  }, [setOptimisticUrl]);

  const { commit } = state;
  // ast-grep-ignore: tsx/use-effect-cleanup
  useLayoutEffect(() => {
    // This only signals that the navigation finished, so there is nothing to clean up.
    commit();
  }, [commit]);

  return { url: state.url, optimisticUrl, isPending };
};

export const RouterProvider = ({ children }: { children: ReactNode }) => (
  <RouterContext value={useNavigationTransitions()}>{children}</RouterContext>
);
