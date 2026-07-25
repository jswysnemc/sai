import { describe, expect, it } from "vitest";
import { ensureGoalPrefix, hasGoalPrefix, toggleGoalMode } from "./goal-control";

describe("goal mode prefix helpers", () => {
  it("toggles goal prefix", () => {
    expect(toggleGoalMode("完成功能")).toBe("/goal 完成功能");
    expect(toggleGoalMode("/goal 完成功能")).toBe("完成功能");
    expect(toggleGoalMode("")).toBe("/goal ");
    expect(hasGoalPrefix("/goal 测试")).toBe(true);
    expect(ensureGoalPrefix("完成")).toBe("/goal 完成");
  });
});
