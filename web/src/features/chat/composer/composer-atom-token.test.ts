import { describe, expect, it } from "vitest";
import { formatTerminalSelection, parseComposerAtoms } from "./composer-atom-token";

describe("composer atom token", () => {
  it("parses file skill and goal tokens in source order", () => {
    expect(parseComposerAtoms("检查 @src/main.rs 使用 /drawio 后执行 /goal 完成重构")).toEqual([
      { type: "text", value: "检查 " },
      { type: "file", path: "src/main.rs", value: "@src/main.rs" },
      { type: "text", value: " 使用 " },
      { type: "skill", name: "drawio", value: "/drawio" },
      { type: "text", value: " 后执行 " },
      { type: "goal", value: "/goal" },
      { type: "text", value: " 完成重构" }
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
});
