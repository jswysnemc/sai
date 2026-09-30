import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { JevExposureCard } from "./jev-exposure-card";

describe("JevExposureCard", () => {
  it("只命中片段与记忆时折叠行列出它们，而不是提示没有新工具", () => {
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
    expect(html).toContain("片段:数据库迁移, 记忆");
    expect(html).toContain("1 个片段 · 记忆");
    expect(html).not.toContain("没有新的工具");
  });

  it("什么都没选中时说明四类都为空", () => {
    const html = renderToStaticMarkup(
      <JevExposureCard phase="empty" exposure={{ tools: [], skills: [], contexts: [] }} />
    );
    expect(html).toContain("没有新的工具、Skill 或上下文");
  });
});
