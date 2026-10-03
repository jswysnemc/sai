import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ToolLifecycle } from "../run-event-reducer";
import { ContextCompressionTool } from "./context-compression-tool";

/**
 * 【上下文】【显示回归】渲染压缩卡片的指定生命周期。
 * @param patch 工具状态及回执覆盖
 * @returns 静态界面标记
 */
function render(patch: Partial<ToolLifecycle>): string {
  const tool: ToolLifecycle = {
    id: "compress-1", name: "compress_context", argumentsPreview: "", arguments: '{"topic":"源码检查","summary":"配置有效"}',
    progress: "", output: "", status: "preparing", ...patch
  };
  return renderToStaticMarkup(<ContextCompressionTool tool={tool} expanded onToggle={() => {}} />);
}

describe("压缩状态卡片", () => {
  it("分别显示摘要生成与执行阶段", () => {
    expect(render({ status: "preparing" })).toContain("正在生成上下文摘要");
    expect(render({ status: "running" })).toContain("正在压缩上下文");
  });
  it("成功后显示实际回执中的正文估算，并说明原文可回读", () => {
    const html = render({ status: "completed", output: '{"before_tokens":12000,"after_tokens":800}' });
    expect(html).toContain("上下文已压缩");
    expect(html).toContain("12,000 → 800 token");
    expect(html).toContain("工具原文已保留");
  });
  it("失败时展示原因，不展示成功或节省提示", () => {
    const html = render({ status: "failed", output: "stale context revision" });
    expect(html).toContain("上下文压缩失败");
    expect(html).toContain("stale context revision");
    expect(html).not.toContain("工具原文已保留");
  });
  it("回执缺少数值时不编造节省量", () => {
    expect(render({ status: "completed", output: "{}" })).not.toContain("token");
  });
});
