import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EngineReadyPart } from "./engine-ready-part";

/**
 * 渲染外部内核连接提示。
 *
 * @param engine ACP 握手返回的内核展示名称
 * @returns 连接提示静态标记
 */
function renderEngine(engine: string): string {
  return renderToStaticMarkup(<EngineReadyPart engine={engine} version="1.0.0" />);
}

describe("EngineReadyPart", () => {
  it("uses the Claude Code brand icon", () => {
    expect(renderEngine("Claude Code")).toContain('data-agent-engine-brand="claude-code"');
  });

  it("uses the Codex brand icon", () => {
    expect(renderEngine("Codex")).toContain('data-agent-engine-brand="codex"');
  });

  it("keeps the generic icon for an unknown ACP agent", () => {
    expect(renderEngine("Custom Agent")).toContain('data-agent-engine-brand="generic"');
  });
});
