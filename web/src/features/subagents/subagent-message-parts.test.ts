import { describe, expect, it } from "vitest";
import { subagentMessageParts } from "./subagent-message-parts";

describe("subagentMessageParts", () => {
  it("converts reasoning, tools and text to the main conversation parts", () => {
    const parts = subagentMessageParts([
      { kind: "reasoning", text: "分析" },
      { kind: "tool", step: 1, name: "read_file", args_preview: "a.rs", ok: true, output_preview: "内容" },
      { kind: "text", text: "结论" }
    ], false, "2026-01-01T00:00:00Z");

    expect(parts.map((part) => part.type)).toEqual(["reasoning", "tool", "text"]);
    expect(parts[1]).toMatchObject({ type: "tool", tool: { name: "read_file", status: "completed" } });
  });
});
