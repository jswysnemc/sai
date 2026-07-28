import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentEngineBrandIcon } from "./agent-engine-brand-icon";

/**
 * 渲染指定内核的品牌图标。
 *
 * @param engine 对话内核标识
 * @returns 图标静态标记
 */
function renderIcon(engine: "native" | "claude_code" | "codex" | "custom"): string {
  return renderToStaticMarkup(<AgentEngineBrandIcon engine={engine} size={16} />);
}

describe("AgentEngineBrandIcon", () => {
  it("renders the Claude Code brand asset", () => {
    const html = renderIcon("claude_code");

    expect(html).toContain('data-agent-engine-brand="claude-code"');
    expect(html).toContain("Claude%20Code");
  });

  it("renders the Codex brand asset", () => {
    const html = renderIcon("codex");

    expect(html).toContain('data-agent-engine-brand="codex"');
    expect(html).toContain("%3eCodex%3c/title%3e");
  });

  it("keeps a generic fallback for non-branded engines", () => {
    const html = renderIcon("native");

    expect(html).toContain('data-agent-engine-brand="generic"');
    expect(html).not.toContain("data:image/svg+xml");
  });
});
