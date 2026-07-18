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
import { detectInitialLocale, text } from "./features/i18n/locale";
import { configureMonacoEnvironment } from "./features/workspace/monaco-environment";

async function start() {
  initializeTheme();
  // 尽早配置 Monaco，避免设置页 JSON 编辑器在未进代码页时触发 toUrl 报错
  configureMonacoEnvironment();
  await bootstrapSession();
  const root = document.getElementById("root");
  if (!root) {
    const locale = detectInitialLocale();
    throw new Error(text(locale, "The root element is missing", "缺少根元素"));
  }
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
  const locale = detectInitialLocale();
  const main = document.createElement("main");
  const title = document.createElement("h1");
  const detail = document.createElement("p");
  main.className = "fatal-error";
  title.textContent = text(locale, "Sai Web could not start", "Sai Web 无法启动");
  detail.textContent = message;
  main.append(title, detail);
  document.body.replaceChildren(main);
});
