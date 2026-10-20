import { cpSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";
import type { Plugin } from "vite-plus";

const require = createRequire(import.meta.url);
const mathjaxDir = dirname(require.resolve("mathjax/tex-chtml.js"));
const fontDir = dirname(
  createRequire(join(mathjaxDir, "package.json")).resolve("@mathjax/mathjax-newcm-font/chtml.js"),
);

/** Bundles the extensions and fonts MathJax loads at runtime. Otherwise MathJax loads them from a CDN. */
const mathjaxFiles = (): Plugin => ({
  name: "conceptory:mathjax-files",
  apply: "build",
  writeBundle: (options) => {
    const out = join(options.dir ?? "dist", "vendor");
    for (const file of ["tex-chtml.js", "a11y/assistive-mml.js"]) {
      cpSync(join(mathjaxDir, file), join(out, "mathjax", file));
    }
    cpSync(join(mathjaxDir, "input/tex/extensions"), join(out, "mathjax/input/tex/extensions"), {
      recursive: true,
    });
    cpSync(join(fontDir, "chtml"), join(out, "mathjax-newcm/chtml"), { recursive: true });
  },
});

/**
 * Proxy target for the API and files during development. Start
 * `conceptory okf view <roots> --no-open --allow-origin http://localhost:5173` separately.
 */
const backend = "http://127.0.0.1:7878";

export default defineConfig(({ command }) => ({
  plugins: [react({ compiler: true }), tailwindcss(), mathjaxFiles()],
  define: {
    // In development, load the files from node_modules through Vite's /@fs/.
    MATHJAX_PATH: JSON.stringify(command === "build" ? "/vendor/mathjax" : `/@fs${mathjaxDir}`),
    MATHJAX_FONT_PATH: JSON.stringify(
      command === "build" ? "/vendor/mathjax-newcm" : `/@fs${fontDir}`,
    ),
  },
  server: {
    proxy: {
      "/api": { target: backend, changeOrigin: true },
      "/files": { target: backend, changeOrigin: true },
    },
  },
  build: {
    // mermaid and MathJax are large, but they load only when a document uses them.
    chunkSizeWarningLimit: 4096,
  },
  run: {
    tasks: {
      build: {
        command: "vp build",
      },
    },
  },
}));
