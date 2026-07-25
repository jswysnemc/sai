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
        "稳定系统提示",
        "模式提醒",
        "当前模型",
        "运行时",
        "关联记忆",
        "工具定义 (17)"
      ]
    }, t);

    expect(tags.filter((tag) => /工具/.test(tag))).toEqual(["工具定义 (17)"]);
    expect(tags).toContain("技能目录");
    expect(tags).toContain("会话 baseline");
    expect(tags).toContain("关联记忆");
    expect(tags.filter((tag) => tag === "关联记忆")).toHaveLength(1);
    expect(tags).not.toContain("动态段");
  });

  it("在 sections 缺失工具定义时回退到工具摘要标签", () => {
    const tags = buildContextPromptTags({
      has_tools: true,
      tool_count: 3,
      sections: ["稳定系统提示"]
    }, t);

    expect(tags).toContain("工具 (3)");
  });
});
