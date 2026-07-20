import { describe, expect, it } from "vitest";
import { formatTerminalSelection, parseComposerAtoms } from "./composer-atom-token";
import { formatFileMention } from "./file-mention-token";
import { formatSkillMention } from "./skill-mention-token";

describe("composer atom token", () => {
  it("special-renders only successful file and skill picks", () => {
    const file = formatFileMention("src/main.rs");
    const skill = formatSkillMention("drawio");
    expect(parseComposerAtoms(`检查 ${file} 使用 ${skill} 后执行 /goal 完成重构，手写 /research 和 @src/other.rs`)).toEqual([
      { type: "text", value: "检查 " },
      { type: "file", path: "src/main.rs", value: file },
      { type: "text", value: " 使用 " },
      { type: "skill", name: "drawio", value: skill },
      { type: "text", value: " 后执行 " },
      { type: "goal", value: "/goal" },
      { type: "text", value: " 完成重构，手写 /research 和 @src/other.rs" }
    ]);
  });

  it("round trips multiline terminal selections", () => {
    const value = formatTerminalSelection("Terminal 1", "line <one>\nline & two");

    expect(parseComposerAtoms(`分析 ${value}`)).toEqual([
      { type: "text", value: "分析 " },
      {
        type: "terminal",
        source: "Terminal 1",
        content: "line <one>\nline & two",
        value
      }
    ]);
  });

  it("parses expanded skill references as one previewable atom", () => {
    const value = "使用 <skill-reference name=\"research\">\n# Research\nRead primary sources\n</skill-reference> 完成分析";

    expect(parseComposerAtoms(value)).toEqual([
      { type: "text", value: "使用 " },
      {
        type: "skill",
        name: "research",
        content: "# Research\nRead primary sources",
        value: "<skill-reference name=\"research\">\n# Research\nRead primary sources\n</skill-reference>"
      },
      { type: "text", value: " 完成分析" }
    ]);
  });

  it("maps skill-mention goal to goal atom instead of skill", () => {
    expect(parseComposerAtoms(`prefix <skill-mention name="goal"></skill-mention> finish`)).toEqual([
      { type: "text", value: "prefix " },
      { type: "goal", value: "/goal" },
      { type: "text", value: " finish" }
    ]);
  });
});
