import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { RuntimeSettingsSection } from "../runtime-settings-section";
import { getSettingsSection, resolveSettingsSubview } from "../settings-registry";
import { RUNTIME_SEARCH_ENTRIES } from "../search/entries-runtime";
import { fieldAnchorId } from "../search/field-anchor";

const config = {
  active_provider: "", providers: [], gateways: {}, agent: { engine: "custom", acp: { command: "adapter", args: ["--stdio"] } },
  tools: { enabled: true, background_commands_enabled: true, background_command_timeout_seconds: 0, background_command_log_max_bytes: 1024, background_command_stop_grace_seconds: 5 },
  display: { reasoning: "full", tool_calls: "summary", readable_tool_names: true, wait_show_model: true, wait_show_thinking_level: true, repl_transcript_row_cap: 5000 }
} as unknown as AppConfig;

/**
 * 渲染经过注册表归一化的运行时子页。
 * @param subview 新旧路由子页标识
 * @returns 静态页面标记
 */
function renderSubview(subview: string): string {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={client}>
      <RuntimeSettingsSection config={config} subview={resolveSettingsSubview(getSettingsSection("runtime"), subview)} onConfigChange={() => undefined} />
    </QueryClientProvider>
  );
}

describe("runtime settings routing", () => {
  it("keeps permissions and terminal fields reachable after the subview merge", () => {
    for (const route of ["environment", "permissions", "terminal"]) {
      const html = renderSubview(route);
      expect(html).toContain("TUI 默认模式");
      expect(html).toContain("终端 Shell");
      expect(html).toContain("TUI 剪贴板粘贴键");
      expect(html).not.toContain("新会话模型");
    }
  });

  it("keeps context settings reachable through the old context URL", () => {
    for (const route of ["tools", "context"]) {
      const html = renderSubview(route);
      expect(html).toContain("自动压缩比例");
      expect(html).toContain("压缩预留 token");
      expect(html).toContain("过滤档位");
      expect(html).toContain("开启 API 调试");
    }
  });

  it("groups new session and title defaults with engine and retry settings", () => {
    const html = renderSubview("execution");
    for (const label of ["新会话模型", "新会话思考等级", "首轮自动标题", "最大尝试次数"]) expect(html).toContain(label);
    expect(html).toContain('value="adapter"');
    expect(html).toContain("--stdio</textarea>");
  });

  it("renders every indexed field on its registered subview", () => {
    const pages = new Map(["execution", "environment", "tools"].map((route) => [route, renderSubview(route)]));
    for (const entry of RUNTIME_SEARCH_ENTRIES) expect(pages.get(entry.subview!), entry.anchor).toContain(`id="${fieldAnchorId(entry.anchor)}"`);
  });
});
