import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { JevExposureCard } from "./jev-exposure-card";

describe("JevExposureCard", () => {
  it("只命中片段与记忆时折叠行用一句话说明，而不是提示没有新工具", () => {
    const html = renderToStaticMarkup(
      <JevExposureCard
        phase="ready"
        exposure={{
          tools: [],
          skills: [],
          contexts: [
            { kind: "prompt", id: "p", source: "system", description: "数据库迁移", preview: "..." },
            { kind: "memory", id: "memory_context", source: "memory", description: "", preview: "" }
          ]
        }}
      />
    );
    expect(html).toContain("选用了 1 个提示词片段和记忆");
    expect(html).not.toContain("发送前");
    expect(html).not.toContain("无需额外");
  });

  it("什么都没选中时说明本轮无需额外资源", () => {
    const html = renderToStaticMarkup(
      <JevExposureCard phase="empty" exposure={{ tools: [], skills: [], contexts: [] }} />
    );
    expect(html).toContain("本轮无需额外的工具或上下文");
  });
});
