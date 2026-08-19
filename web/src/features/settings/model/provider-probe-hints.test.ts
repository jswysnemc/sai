import { describe, expect, it } from "vitest";
import { stageLabel } from "./provider-probe-hints";

/** 测试里只取英文文案。 */
const t = (en: string, _zh: string) => en;

describe("stageLabel", () => {
  it("只翻译仍在使用的探测阶段", () => {
    expect(stageLabel("completion", t)).toBe("Model response");
    expect(stageLabel("tool_call", t)).toBe("Tool calling");
    expect(stageLabel("catalog", t)).toBe("catalog");
  });
});
