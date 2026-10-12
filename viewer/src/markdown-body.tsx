import { useLayoutEffect, useRef } from "react";

import { enhance } from "./enhance.ts";
import { scrollToHash } from "./route.ts";

/** Scrolls to the heading the URL's # points to when a document is first shown. */
const scrollOnMount = (): (() => void) => {
  const frame = requestAnimationFrame(() => {
    scrollToHash(globalThis.location.hash);
  });
  return () => {
    cancelAnimationFrame(frame);
  };
};

/**
 * Shows the sanitized body and adds code highlighting, diagrams, and math.
 *
 * React doesn't manage the body, which is replaced only when the HTML changes, so re-renders
 * don't erase the added rendering.
 */
export const MarkdownBody = ({ html }: { html: string }) => {
  const ref = useRef<HTMLDivElement>(null);

  // The dependency array is an optimization to avoid re-enhancing the same body on every render.
  useLayoutEffect(() => {
    const container = ref.current;
    if (container === null) {
      return () => {};
    }
    container.innerHTML = html;
    let cancelled = false;
    void enhance({ container, cancelled: () => cancelled });
    return () => {
      cancelled = true;
    };
  }, [html]);

  return (
    <div ref={scrollOnMount}>
      <div ref={ref} className="markdown-body" />
    </div>
  );
};
