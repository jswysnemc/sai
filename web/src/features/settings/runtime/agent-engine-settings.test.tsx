import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { I18nProvider } from "../../i18n/i18n-context";
import { AgentEngineSettings } from "./agent-engine-settings";

/**
 * 渲染内核设置为静态标记。
 *
 * @param agent 内核配置片段
 * @returns 渲染出的 HTML
 */
function render(agent: Record<string, unknown>): string {
  const config = { agent } as unknown as AppConfig;
  return renderToStaticMarkup(
    <I18nProvider>
      <AgentEngineSettings config={config} onConfigChange={() => undefined} />
    </I18nProvider>
  );
}

describe("AgentEngineSettings", () => {
  it("does not warn about disabled features on the native engine", () => {
    const html = render({ engine: "native" });

    expect(html).not.toContain("停用");
  });

  /// 切到外部内核时，压缩与记忆会静默失效，界面必须说清楚。
  it("lists the features an external engine disables", () => {
    const html = render({ engine: "codex" });

    expect(html).toContain("停用");
    expect(html).toContain("上下文压缩");
    expect(html).toContain("记忆注入");
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
