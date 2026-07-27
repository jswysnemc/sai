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

  /// 外部内核配置应提供标准 ACP 配置入口，不再硬编码禁用能力。
  it("shows ACP config options for an external engine", () => {
    const html = render({ engine: "codex" });

    expect(html).toContain("ACP 模型");
    expect(html).toContain("ACP 权限模式");
    expect(html).not.toContain("上下文压缩");
  });

  it("asks for a launch command only on the custom engine", () => {
    expect(render({ engine: "codex" })).not.toContain("启动命令");
    expect(render({ engine: "custom" })).toContain("启动命令");
  });

  it("treats a missing agent section as the native engine", () => {
    const html = render({});

    expect(html).not.toContain("停用");
  });
});
