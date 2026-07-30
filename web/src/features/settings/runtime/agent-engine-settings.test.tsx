import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { AgentEngineSettings } from "./agent-engine-settings";

/**
 * 渲染内核设置为静态标记。
 *
 * 刻意不包 I18nProvider：它会调用 detectInitialLocale 读取浏览器语言，
 * 在中文环境与 CI 的英文环境下渲染出不同文案，断言随之飘移。
 * 不带 Provider 时 useI18n 取固定为 zh-CN 的 fallback，
 * 结果与运行环境无关——项目里其它组件测试也是这个约定。
 *
 * @param agent 内核配置片段
 * @returns 渲染出的 HTML
 */
function render(agent: Record<string, unknown>): string {
  const config = { agent } as unknown as AppConfig;
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <AgentEngineSettings config={config} onConfigChange={() => undefined} />
    </QueryClientProvider>
  );
}

describe("AgentEngineSettings", () => {
  it("does not warn about disabled features on the native engine", () => {
    const html = render({ engine: "native" });

    expect(html).not.toContain("停用");
  });

  /// 外部内核设置页只保留连接类配置，运行参数已移到输入区。
  it("keeps only connection settings for an external engine", () => {
    const html = render({ engine: "codex" });

    expect(html).toContain("ACP 认证方式");
    expect(html).toContain("附加目录");
    expect(html).toContain('data-agent-engine-brand="codex"');
    // 模型、思考等级与权限模式改由输入区调整，不再挤占设置页
    expect(html).not.toContain("ACP 模型");
    expect(html).not.toContain("ACP 权限模式");
    expect(html).not.toContain("ACP 思考等级");
    expect(html).not.toContain("上下文压缩");
  });

  it("shows the Claude Code brand icon when that engine is selected", () => {
    const html = render({ engine: "claude_code" });

    expect(html).toContain('data-agent-engine-brand="claude-code"');
  });

  it("asks for a launch command only on the custom engine", () => {
    expect(render({ engine: "codex" })).not.toContain("启动命令");
    expect(render({ engine: "custom" })).toContain("启动命令");
  });

  it("treats a missing agent section as the native engine", () => {
    const html = render({});

    expect(html).not.toContain("停用");
  });

  it("shows model and reasoning defaults for future sessions", () => {
    const html = render({ engine: "native" });

    expect(html).toContain("新会话模型");
    expect(html).toContain("新会话思考等级");
    expect(html).toContain("跟随内核默认模型");
  });
});
