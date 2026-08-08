import { describe, expect, it } from "vitest";
import { formatToolDuration, toolDurationLabel } from "./tool-duration";

describe("formatToolDuration", () => {
  it("毫秒级取整数毫秒", () => {
    expect(formatToolDuration(320)).toBe("320ms");
    expect(formatToolDuration(999)).toBe("999ms");
  });

  it("秒级保留一位小数", () => {
    expect(formatToolDuration(1_000)).toBe("1.0s");
    expect(formatToolDuration(12_340)).toBe("12.3s");
  });

  it("分钟级补分钟前缀", () => {
    expect(formatToolDuration(60_000)).toBe("1m");
    expect(formatToolDuration(130_000)).toBe("2m10s");
  });

  it("秒数取整满 60 时进位到分钟", () => {
    expect(formatToolDuration(119_600)).toBe("2m");
  });

  it("无效时长返回空串", () => {
    expect(formatToolDuration(Number.NaN)).toBe("");
    expect(formatToolDuration(-5)).toBe("");
  });
});

describe("toolDurationLabel", () => {
  it("已结束的调用取起止差值", () => {
    expect(toolDurationLabel(1_000, 2_500, 9_999)).toBe("1.5s");
  });

  it("执行中的调用以当前时间做差", () => {
    expect(toolDurationLabel(1_000, undefined, 3_000)).toBe("2.0s");
  });

  it("过短的调用不展示耗时", () => {
    expect(toolDurationLabel(1_000, 1_100, 1_100)).toBe("");
  });

  it("缺少起点时返回空串", () => {
    expect(toolDurationLabel(undefined, 5_000, 5_000)).toBe("");
  });
});
