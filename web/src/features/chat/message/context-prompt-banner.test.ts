import { describe, expect, it } from "vitest";
import { buildContextPromptTags } from "./context-prompt-banner";

const t = (en: string, zh: string) => zh;

describe("buildContextPromptTags", () => {
  it("不重复展示工具摘要与工具定义标签", () => {
    const tags = buildContextPromptTags({
      source: "session_baseline",
      has_skills: true,
      has_memory: true,
      has_dynamic: true,
      has_tools: true,
      tool_count: 17,
      sections: [
        { id: "baseline", label: "会话 baseline", content: "baseline" },
        { id: "skills", label: "技能目录", content: "skills" },
        { id: "mode", label: "模式说明", content: "mode" },
        { id: "runtime", label: "运行时", content: "runtime" },
        { id: "memory", label: "关联记忆", content: "memory" },
        { id: "tools", label: "工具定义 (17)", content: "tools" }
      ]
    }, t);

    const labels = tags.map((tag) => tag.label);
    expect(labels.filter((tag) => /工具/.test(tag))).toEqual(["工具定义 (17)"]);
    expect(labels).toContain("技能目录");
    expect(labels).toContain("会话 baseline");
    expect(labels).toContain("关联记忆");
    expect(labels.filter((tag) => tag === "关联记忆")).toHaveLength(1);
    expect(labels).not.toContain("动态段");
  });

  it("在 sections 缺失工具定义时回退到工具摘要标签", () => {
    const tags = buildContextPromptTags({
      has_tools: true,
      tool_count: 3,
      sections: [{ id: "baseline", label: "稳定系统提示", content: "baseline" }]
    }, t);

    expect(tags.map((tag) => tag.label)).toContain("工具 (3)");
  });
});
