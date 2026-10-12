// Draws links and supersession between documents with Cytoscape.js.
//
// Elements are added in an effect and removed in its cleanup. Node positions are saved before
// removal and restored on the next add, so the layout survives changes to the overview.

import cytoscape from "cytoscape";
import type { Core, LayoutOptions, NodeSingular, Position, StylesheetJson } from "cytoscape";
import fcose from "cytoscape-fcose";
import type { FcoseLayoutOptions } from "cytoscape-fcose";
import { useEffect, useLayoutEffect, useState } from "react";

import type { Overview } from "./data.ts";
import { href } from "./route.ts";

cytoscape.use(fcose);

export type GraphMode = "local" | "global";

type GraphNode = {
  id: string;
  label: string;
  type: string;
  deprecated: boolean;
};

type GraphEdge = {
  id: string;
  source: string;
  target: string;
  kind: string;
};

type GraphData = {
  mode: GraphMode;
  focused: string | null;
  nodes: GraphNode[];
  edges: GraphEdge[];
};

/** The local graph draws only the documents linked to or from the current document. */
const graphOf = ({
  overview,
  focused,
  mode,
}: {
  overview: Overview;
  focused: string | null;
  mode: GraphMode;
}): GraphData => {
  const concepts = overview.documents.filter((document) => document.kind === "concept");
  const conceptIds = new Set(concepts.map((document) => document.id));
  const edges = overview.edges.filter(
    (edge) => conceptIds.has(edge.source) && conceptIds.has(edge.target),
  );
  const local = mode === "local" && focused !== null && conceptIds.has(focused);
  const shown = local
    ? new Set([
        focused,
        ...edges.filter((edge) => edge.source === focused).map((edge) => edge.target),
        ...edges.filter((edge) => edge.target === focused).map((edge) => edge.source),
      ])
    : conceptIds;
  return {
    mode: local ? "local" : "global",
    focused,
    nodes: concepts
      .filter((document) => shown.has(document.id))
      .map((document) => ({
        id: document.id,
        label: document.title,
        type: document.type ?? "",
        deprecated: document.deprecated_by !== null,
      })),
    edges: edges
      .filter((edge) => shown.has(edge.source) && shown.has(edge.target))
      .map((edge) => ({
        id: `${edge.kind}:${edge.source}->${edge.target}`,
        source: edge.source,
        target: edge.target,
        kind: edge.kind,
      })),
  };
};

/** Node positions saved before removal, kept separately for the local and global graphs. */
const savedPositions = new Map<string, Position>();
const positionKey = ({ mode, id }: { mode: GraphMode; id: string }): string => `${mode}:${id}`;

const runLayout = ({ cy, randomize }: { cy: Core; randomize: boolean }): void => {
  const layout: FcoseLayoutOptions = {
    name: "fcose",
    randomize,
    animate: false,
    nodeSeparation: 80,
    idealEdgeLength: () => 80,
    // Lay out nodes by their size including labels so titles don't overlap.
    nodeDimensionsIncludeLabels: true,
    padding: 16,
  };
  // @types/cytoscape-fcose doesn't add the fcose options to LayoutOptions.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  cy.layout(layout as unknown as LayoutOptions).run();
};

const showGraph = ({ cy, graph }: { cy: Core; graph: GraphData }): void => {
  let unplaced = 0;
  cy.startBatch();
  for (const data of graph.nodes) {
    const saved = savedPositions.get(positionKey({ mode: graph.mode, id: data.id }));
    if (saved === undefined) {
      unplaced += 1;
    }
    cy.add({ group: "nodes", data, ...(saved === undefined ? {} : { position: saved }) });
  }
  for (const data of graph.edges) {
    cy.add({ group: "edges", data });
  }
  if (graph.focused !== null) {
    cy.getElementById(graph.focused).addClass("focused");
  }
  cy.endBatch();
  if (unplaced > 0) {
    // Lay out from scratch if no node has a position yet; otherwise start from the current layout.
    runLayout({ cy, randomize: unplaced === graph.nodes.length });
  }
  cy.fit(undefined, 16);
  if (cy.zoom() > 1.5) {
    cy.zoom({ level: 1.5, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
  }
};

const hideGraph = ({ cy, mode }: { cy: Core; mode: GraphMode }): void => {
  for (const node of cy.nodes()) {
    savedPositions.set(positionKey({ mode, id: node.id() }), { ...node.position() });
  }
  cy.elements().remove();
};

/** Reads a color defined in CSS. */
const themeColor = (name: string): string =>
  getComputedStyle(document.documentElement).getPropertyValue(`--color-${name}`).trim();

const nodeStyle = (color: (name: string) => string): StylesheetJson => [
  {
    selector: "node",
    style: {
      label: "data(label)",
      "font-size": 10,
      "font-family": "system-ui, sans-serif",
      color: color("fg-muted"),
      "text-valign": "bottom",
      "text-margin-y": 4,
      "text-wrap": "ellipsis",
      "text-max-width": "120px",
      "background-color": color("accent"),
      width: 10,
      height: 10,
    },
  },
  {
    selector: 'node[type = "StyleGuide"]',
    style: { "background-color": color("guide"), shape: "round-rectangle" },
  },
  {
    selector: "node[?deprecated]",
    style: { "background-color": color("line-strong") },
  },
  {
    selector: "node.focused",
    style: { width: 14, height: 14, color: color("fg"), "font-weight": "bold" },
  },
];

const edgeStyle = (color: (name: string) => string): StylesheetJson => [
  {
    selector: "edge",
    style: {
      width: 1,
      "line-color": color("line"),
      "target-arrow-color": color("line"),
      "target-arrow-shape": "triangle",
      "curve-style": "bezier",
      "arrow-scale": 0.6,
    },
  },
  {
    selector: 'edge[kind = "deprecated-by"]',
    style: {
      "line-style": "dashed",
      "line-color": color("danger"),
      "target-arrow-color": color("danger"),
    },
  },
];

const graphStyle = (): StylesheetJson => [...nodeStyle(themeColor), ...edgeStyle(themeColor)];

/** Returns the Cytoscape.js instance and a ref callback for its container element. */
const useCytoscape = () => {
  const [cy, setCy] = useState<Core | null>(null);

  /** Starts Cytoscape.js when the element is attached and destroys it when detached. */
  const attach = (container: HTMLDivElement): (() => void) => {
    const instance = cytoscape({ container, style: graphStyle(), minZoom: 0.2, maxZoom: 3 });
    instance.on("tap", "node", (event) => {
      // The Cytoscape.js types declare the event target as any.
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      const node = event.target as NodeSingular;
      void navigation.navigate(href({ view: "document", id: node.id() }));
    });
    setCy(instance);
    return () => {
      setCy(null);
      instance.destroy();
    };
  };

  // Colors come from CSS variables, so re-read them when the color scheme changes.
  useEffect(() => {
    if (cy === null) {
      return () => {};
    }
    const scheme = globalThis.matchMedia("(prefers-color-scheme: dark)");
    const restyle = (): void => {
      cy.style(graphStyle());
    };
    scheme.addEventListener("change", restyle);
    return () => {
      scheme.removeEventListener("change", restyle);
    };
  }, [cy]);

  return { cy, attach };
};

type Props = {
  overview: Overview;
  /** The document to highlight. The local graph draws only the documents linked to or from it. */
  focused: string | null;
  mode: GraphMode;
};

export const Graph = ({ overview, focused, mode }: Props) => {
  const { cy, attach } = useCytoscape();
  const graph = graphOf({ overview, focused, mode });

  useLayoutEffect(() => {
    if (cy === null) {
      return () => {};
    }
    showGraph({ cy, graph });
    return () => {
      hideGraph({ cy, mode: graph.mode });
    };
  }, [cy, graph]);

  // The side panel's link lists show the same content, so hide the canvas from assistive tech.
  return (
    <div ref={attach} aria-hidden="true" className="h-72 rounded-md border border-line bg-canvas" />
  );
};
