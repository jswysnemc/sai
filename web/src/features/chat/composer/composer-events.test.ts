import { describe, expect, it } from "vitest";
import { parseComposerAtoms } from "./composer-atom-token";
import { appendTerminalSelection } from "./composer-events";

describe("appendTerminalSelection", () => {
  it("appends one structured terminal atom with spacing", () => {
    const value = appendTerminalSelection("分析结果", {
      source: "Build",
      content: "error: failed\nexit 1"
    });

    expect(parseComposerAtoms(value)).toEqual([
      { type: "text", value: "分析结果 " },
      expect.objectContaining({ type: "terminal", source: "Build", content: "error: failed\nexit 1" }),
      { type: "text", value: " " }
    ]);
  });
});
