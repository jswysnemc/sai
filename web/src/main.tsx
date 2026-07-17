import "@fontsource/fira-sans/latin-400.css";
import "@fontsource/fira-sans/latin-500.css";
import "@fontsource/fira-sans/latin-600.css";
import "@fontsource/fira-code/latin-400.css";
import "@fontsource/fira-code/latin-500.css";
import "@xterm/xterm/css/xterm.css";
import "katex/dist/katex.min.css";
import "./shared/styles/tokens.css";
import "./shared/styles/global.css";
import "./shared/styles/scrollbar.css";
import "./shared/styles/surfaces.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./app/app";
import { queryClient } from "./app/query-client";
import { bootstrapSession } from "./api/client";
import { initializeTheme } from "./features/theme/theme";

async function start() {
  initializeTheme();
  await bootstrapSession();
  const root = document.getElementById("root");
  if (!root) throw new Error("root element is missing");
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </QueryClientProvider>
    </StrictMode>
  );
}

start().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  document.body.innerHTML = `<main class="fatal-error"><h1>Sai Web 无法启动</h1><p>${message}</p></main>`;
});
