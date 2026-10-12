// Adds the rendering GitHub applies in the browser to the body HTML the server returns.
//
// starry-night highlights code, mermaid draws diagrams, and MathJax renders math. All of them are
// large, so each loads only when a document first needs it.

import type { createStarryNight } from "@wooorm/starry-night";
import type mermaidModule from "mermaid";
import onigurumaUrl from "vscode-oniguruma/release/onig.wasm?url";

type Target = {
  container: HTMLElement;
  /** Returns whether to stop. Work on an old body stops once the body is replaced. */
  cancelled: () => boolean;
};

const special = new Set(["mermaid", "math"]);

type StarryNight = Awaited<ReturnType<typeof createStarryNight>>;
let starryNight: Promise<StarryNight> | null = null;

/** Loads on the first call and returns the same Promise afterwards. */
const loadStarryNight = (): Promise<StarryNight> => {
  // starry-night fetches the regex engine's wasm from a CDN by default, so pass the bundled one.
  starryNight ??= (async () => {
    const { common, createStarryNight: create } = await import("@wooorm/starry-night");
    return create(common, {
      getOnigurumaUrlFetch: () => new URL(onigurumaUrl, globalThis.location.href),
    });
  })();
  return starryNight;
};

/** The part of the hast from starry-night that the conversion uses. */
type HastParent = {
  children: HastNode[];
};

type HastNode =
  | { type: "text"; value: string }
  | ({ type: "element"; tagName: string; properties: Record<string, unknown> } & HastParent)
  | { type: "comment" | "doctype" };

/** Converts the hast from starry-night to DOM. Its only elements are spans with classes. */
const toDom = (node: HastParent): Node[] =>
  node.children.map((child) => {
    if (child.type === "text") {
      return document.createTextNode(child.value);
    }
    if (child.type === "element") {
      const element = document.createElement(child.tagName);
      const className = child.properties["className"];
      if (Array.isArray(className)) {
        element.className = className.join(" ");
      }
      element.append(...toDom(child));
      return element;
    }
    return document.createTextNode("");
  });

const highlight = async ({ container, cancelled }: Target): Promise<void> => {
  const blocks = [...container.querySelectorAll<HTMLElement>("pre[lang]")].filter(
    (pre) => !special.has(pre.getAttribute("lang") ?? ""),
  );
  if (blocks.length === 0) {
    return;
  }
  const highlighter = await loadStarryNight();
  if (cancelled()) {
    return;
  }
  for (const pre of blocks) {
    const code = pre.querySelector("code");
    const scope = highlighter.flagToScope(pre.getAttribute("lang") ?? "");
    if (code !== null && scope !== undefined) {
      const tree = highlighter.highlight(code.textContent, scope) as HastParent;
      code.replaceChildren(...toDom(tree));
    }
  }
};

type Mermaid = typeof mermaidModule;
let mermaid: Promise<Mermaid> | null = null;
let diagramCount = 0;

const loadMermaid = (): Promise<Mermaid> => {
  mermaid ??= (async () => {
    const { default: instance } = await import("mermaid");
    return instance;
  })();
  return mermaid;
};

const renderMermaid = async ({ container, cancelled }: Target): Promise<void> => {
  const blocks = [...container.querySelectorAll<HTMLElement>('pre[lang="mermaid"]')];
  if (blocks.length === 0) {
    return;
  }
  const instance = await loadMermaid();
  const dark = globalThis.matchMedia("(prefers-color-scheme: dark)").matches;
  instance.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: dark ? "dark" : "default",
  });
  for (const pre of blocks) {
    if (cancelled()) {
      return;
    }
    const figure = document.createElement("div");
    figure.className = "mermaid-diagram";
    try {
      diagramCount += 1;
      // Render diagrams one at a time, or the temporary elements mermaid uses collide.
      // oxlint-disable-next-line no-await-in-loop
      const { svg } = await instance.render(`mermaid-${diagramCount.toString()}`, pre.textContent);
      figure.innerHTML = svg;
      pre.replaceWith(figure);
    } catch (error) {
      // Report syntax errors while keeping the original code.
      figure.className = "mermaid-error";
      figure.textContent = error instanceof Error ? error.message : String(error);
      pre.after(figure);
    }
  }
};

type MathJaxApi = {
  startup: {
    promise: Promise<void>;
    document: { clear: () => void; updateDocument: () => void };
  };
  tex2chtmlPromise: (tex: string, options: { display: boolean }) => Promise<HTMLElement>;
};

declare global {
  // oxlint-disable-next-line typescript/consistent-type-definitions -- augmenting a built-in type requires an interface
  interface Window {
    MathJax?: MathJaxApi | Record<string, unknown>;
  }
}

declare const MATHJAX_PATH: string;
declare const MATHJAX_FONT_PATH: string;

let mathjax: Promise<MathJaxApi> | null = null;

const loadMathJax = (): Promise<MathJaxApi> => {
  mathjax ??= new Promise((resolve, reject) => {
    globalThis.window.MathJax = {
      loader: { paths: { mathjax: MATHJAX_PATH, "mathjax-newcm": MATHJAX_FONT_PATH } },
      startup: { typeset: false },
      // Speech generation hangs trying to load speech-rule-engine, which isn't bundled, so it's
      // disabled. Hidden MathML is attached instead for assistive technology. These options also
      // serve as the menu's defaults, so they must be set here even with the menu disabled.
      options: {
        enableMenu: false,
        menuOptions: {
          settings: {
            enrich: false,
            speech: false,
            braille: false,
            collapsible: false,
            assistiveMml: true,
            inTabOrder: false,
          },
        },
      },
    };
    const script = document.createElement("script");
    script.src = `${MATHJAX_PATH}/tex-chtml.js`;
    script.async = true;
    script.addEventListener("load", () => {
      // Once loaded, MathJax replaces the config object with its API object.
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      const api = globalThis.window.MathJax as MathJaxApi;
      resolve(
        (async () => {
          await api.startup.promise;
          return api;
        })(),
      );
    });
    script.addEventListener("error", () => {
      reject(new Error("Failed to load MathJax"));
    });
    document.head.append(script);
  });
  return mathjax;
};

const renderMath = async ({ container, cancelled }: Target): Promise<void> => {
  const nodes = [...container.querySelectorAll<HTMLElement>("[data-math-style]")];
  if (nodes.length === 0) {
    return;
  }
  const api = await loadMathJax();
  for (const node of nodes) {
    if (cancelled()) {
      return;
    }
    const display = node.dataset["mathStyle"] === "display";
    try {
      // MathJax converts serially, so awaiting all at once wouldn't be faster.
      // oxlint-disable-next-line no-await-in-loop
      node.replaceWith(await api.tex2chtmlPromise(node.textContent.trim(), { display }));
    } catch (error) {
      console.error(error);
    }
  }
  // Add the fonts and styles the converted math needs to the page.
  api.startup.document.clear();
  api.startup.document.updateDocument();
};

export const enhance = async (target: Target): Promise<void> => {
  await Promise.all([highlight(target), renderMermaid(target), renderMath(target)]);
};
