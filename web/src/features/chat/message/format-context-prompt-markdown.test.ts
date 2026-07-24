import { describe, expect, it } from "vitest";
import { formatContextPromptMarkdown } from "./format-context-prompt-markdown";

describe("formatContextPromptMarkdown", () => {
  it("turns instruction-files xml into readable sections", () => {
    const source = [
      "You are a coding agent.",
      "",
      "<instruction-files>",
      "Additional instructions from global and project instruction files.",
      "",
      '<instruction-file scope="project" path="/repo/AGENTS.md">',
      "## Rules",
      "- Prefer small files",
      "</instruction-file>",
      "</instruction-files>",
      "",
      "## Available tools",
      "",
      "### `read_file`",
      "",
      "Read a file"
    ].join("\n");

    const renderedZh = formatContextPromptMarkdown(source, "zh-CN");
    expect(renderedZh).toContain("## 指令文件");
    expect(renderedZh).toContain("### /repo/AGENTS.md");
    expect(renderedZh).toContain("范围：`项目`");
    expect(renderedZh).toContain("## Rules");
    expect(renderedZh).toContain("## Available tools");
    expect(renderedZh).not.toContain("<instruction-files>");
    expect(renderedZh).not.toContain("<instruction-file");

    const renderedEn = formatContextPromptMarkdown(source, "en-US");
    expect(renderedEn).toContain("## Instruction files");
    expect(renderedEn).toContain("Scope: `project`");
    expect(renderedEn).not.toContain("## 指令文件");
  });

  it("keeps plain markdown tools section intact", () => {
    const source = "## Available tools\n\n### `run_command`\n\nRun shell";
    expect(formatContextPromptMarkdown(source, "en-US")).toContain("### `run_command`");
  });

  it("localizes known xml section titles", () => {
    const source = "<available-skills>\n- skill-a\n</available-skills>";
    expect(formatContextPromptMarkdown(source, "en-US")).toContain("## Available skills");
    expect(formatContextPromptMarkdown(source, "zh-CN")).toContain("## 技能目录");
  });
});
