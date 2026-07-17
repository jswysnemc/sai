import { describe, expect, it } from "vitest";
import { THINKING_OPTIONS, thinkingLevelLabel } from "./model-thinking-options";

describe("model thinking options", () => {
  it("为全部推理等级提供唯一选项", () => {
    expect(new Set(THINKING_OPTIONS.map((option) => option.value)).size).toBe(7);
  });

  it("返回英文等级 token", () => {
    expect(thinkingLevelLabel("xhigh")).toBe("xhigh");
    expect(thinkingLevelLabel("high")).toBe("high");
    expect(thinkingLevelLabel("none")).toBe("none");
  });
});
