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
import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./app/app";
import { queryClient } from "./app/query-client";
import { bootstrapSession, fetchAuthMode, hasActiveSession } from "./api/client";
import { PasswordLogin } from "./features/auth/password-login";
import { initializeTheme } from "./features/theme/theme";
import { detectInitialLocale, text } from "./features/i18n/locale";
import { configureMonacoEnvironment } from "./features/workspace/monaco-environment";
import { enableAutoHideScrollbars } from "./shared/styles/auto-hide-scrollbar";

/**
 * 按认证状态在登录页与工作台之间切换。
 *
 * @param props 初始是否已通过验证
 * @returns 登录页或工作台
 */
function Root({ authenticated }: { authenticated: boolean }) {
  const [ready, setReady] = useState(authenticated);
  if (!ready) return <PasswordLogin onAuthenticated={() => setReady(true)} />;
  return (
    <BrowserRouter>
      <App />
    </BrowserRouter>
  );
}

async function start() {
  initializeTheme();
  // 尽早配置 Monaco，避免设置页 JSON 编辑器在未进代码页时触发 toUrl 报错
  configureMonacoEnvironment();
  // 滚动条默认隐藏，滚动/悬停时短暂显示
  enableAutoHideScrollbars();
  await bootstrapSession();

  // 启用口令验证且尚无有效会话时先呈现登录页
  const mode = await fetchAuthMode().catch(() => ({ password_required: false }));
  const authenticated = mode.password_required ? await hasActiveSession() : true;

  const root = document.getElementById("root");
  if (!root) {
    const locale = detectInitialLocale();
    throw new Error(text(locale, "The root element is missing", "缺少根元素"));
  }
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <Root authenticated={authenticated} />
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
