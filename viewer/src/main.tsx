import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app.tsx";
import { LiveProvider } from "./live.tsx";
import { RouterProvider } from "./router.tsx";

import "./styles.css";
import "github-markdown-css/github-markdown.css";
import "@wooorm/starry-night/style/both";

const root = document.querySelector("#root");
if (root !== null) {
  createRoot(root).render(
    <StrictMode>
      <RouterProvider>
        <LiveProvider>
          <App />
        </LiveProvider>
      </RouterProvider>
    </StrictMode>,
  );
}
