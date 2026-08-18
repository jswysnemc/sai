import { describe, expect, it } from "vitest";
import {
  clampRatioPercent,
  clampReserveTokens,
  computeTriggerTokens,
  parseReserveInput,
  resolveTriggerBreakdown
} from "./compaction-trigger";

// 下列期望值与后端 src/state/compaction/budget.rs 的单元测试逐条对应，
// 两边任一侧改了算法，这组断言就会先失败。
describe("computeTriggerTokens", () => {
  it("小窗口按比例触发，不被固定预留拖早", () => {
    expect(computeTriggerTokens(20_000, 0.9, 50_000)).toBe(18_000);
    expect(computeTriggerTokens(80_000, 0.9, 50_000)).toBe(72_000);
    expect(computeTriggerTokens(200_000, 0.9, 50_000)).toBe(180_000);
  });

  it("大窗口按预留空位触发，比九成更晚", () => {
    expect(computeTriggerTokens(1_000_000, 0.9, 50_000)).toBe(950_000);
  });

  it("预留超过窗口或为 0 时回退到纯比例", () => {
    expect(computeTriggerTokens(1_000, 0.9, 50_000)).toBe(900);
    expect(computeTriggerTokens(100, 0.9, 50_000)).toBe(90);
    expect(computeTriggerTokens(200_000, 0.9, 0)).toBe(180_000);
  });

  it("收紧预留可以让中等窗口更晚压缩", () => {
    expect(computeTriggerTokens(32_000, 0.9, 8_000)).toBe(28_800);
    expect(computeTriggerTokens(80_000, 0.9, 8_000)).toBe(72_000);
  });

  it("窗口未知时返回 0", () => {
    expect(computeTriggerTokens(0, 0.9, 50_000)).toBe(0);
  });
});

describe("resolveTriggerBreakdown", () => {
  it("大窗口下由预留条件决定触发点", () => {
    const result = resolveTriggerBreakdown(1_048_576, 0.9, 50_000);
    expect(result.active).toBe("reserve");
    expect(result.ratioTrigger).toBe(943_718);
    expect(result.reserveTrigger).toBe(998_576);
    expect(result.trigger).toBe(998_576);
  });

  it("小窗口下由比例条件决定触发点", () => {
    const result = resolveTriggerBreakdown(80_000, 0.9, 8_000);
    expect(result.active).toBe("ratio");
    expect(result.trigger).toBe(72_000);
  });

  it("预留关闭时该条件不参与", () => {
    const result = resolveTriggerBreakdown(200_000, 0.9, 0);
    expect(result.reserveTrigger).toBeNull();
    expect(result.active).toBe("ratio");
  });

  it("预留大到吃掉整个窗口时退回纯比例", () => {
    const result = resolveTriggerBreakdown(1_000, 0.9, 50_000);
    expect(result.reserveTrigger).toBeNull();
    expect(result.trigger).toBe(900);
  });

  it("两个条件相等时归给比例", () => {
    const result = resolveTriggerBreakdown(100_000, 0.9, 10_000);
    expect(result.ratioTrigger).toBe(90_000);
    expect(result.reserveTrigger).toBe(90_000);
    expect(result.active).toBe("ratio");
  });
});

describe("clampRatioPercent", () => {
  it("夹紧到 50~99", () => {
    expect(clampRatioPercent(30)).toBe(50);
    expect(clampRatioPercent(120)).toBe(99);
    expect(clampRatioPercent(85.4)).toBe(85);
  });

  it("非法输入回落到默认值", () => {
    expect(clampRatioPercent(Number.NaN)).toBe(90);
  });
});

describe("clampReserveTokens", () => {
  it("负数与非法值归零", () => {
    expect(clampReserveTokens(-1, 100_000)).toBe(0);
    expect(clampReserveTokens(Number.NaN, 100_000)).toBe(0);
  });

  it("不超过窗口上限", () => {
    expect(clampReserveTokens(500_000, 100_000)).toBe(100_000);
  });

  it("窗口未知时不设上限", () => {
    expect(clampReserveTokens(500_000, 0)).toBe(500_000);
  });
});

describe("parseReserveInput", () => {
  it("接受纯数字与千分位", () => {
    expect(parseReserveInput("50000")).toBe(50_000);
    expect(parseReserveInput("50,000")).toBe(50_000);
  });

  it("接受 k / m 后缀", () => {
    expect(parseReserveInput("8k")).toBe(8_000);
    expect(parseReserveInput("1.5K")).toBe(1_500);
    expect(parseReserveInput("1m")).toBe(1_000_000);
  });

  it("空串视为关闭预留", () => {
    expect(parseReserveInput("  ")).toBe(0);
  });

  it("无法解析时返回 null", () => {
    expect(parseReserveInput("abc")).toBeNull();
    expect(parseReserveInput("8kk")).toBeNull();
  });
});
