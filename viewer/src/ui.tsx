import type { ReactNode } from "react";

/** Skeleton shown while the body is loading. */
export const PageSkeleton = () => (
  <div aria-busy="true" aria-label="読み込み中" className="animate-pulse space-y-4">
    <div className="h-8 w-2/3 rounded bg-canvas-subtle" />
    <div className="h-4 w-full rounded bg-canvas-subtle" />
    <div className="h-4 w-5/6 rounded bg-canvas-subtle" />
    <div className="h-4 w-3/4 rounded bg-canvas-subtle" />
  </div>
);

const calloutStyles = {
  danger: "border-danger bg-danger-subtle",
  attention: "border-attention bg-attention-subtle",
} as const;

export const Callout = ({
  tone,
  children,
}: {
  tone: keyof typeof calloutStyles;
  children: ReactNode;
}) => (
  <div role="note" className={`mb-4 rounded-md border px-4 py-2 text-sm ${calloutStyles[tone]}`}>
    {children}
  </div>
);

/** Heading for the side panel and lists. */
export const SectionHeading = ({ children }: { children: ReactNode }) => (
  <h2 className="mb-1 text-xs font-semibold text-fg-muted">{children}</h2>
);
