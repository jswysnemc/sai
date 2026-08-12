import { describe, expect, it } from "vitest";
import { isSameModelSelection, resolveModelSelect } from "./pending-model-selection";

const bigPickle = { providerId: "openai", model: "big-pickle" };
const deepseek = { providerId: "deepseek", model: "deepseek-v4-flash" };

describe("isSameModelSelection", () => {
  it("供应商与模型都一致时视为同一选择", () => {
    expect(isSameModelSelection(bigPickle, { ...bigPickle })).toBe(true);
  });

  it.each([
    ["模型不同", { providerId: "openai", model: "small-pickle" }],
    ["供应商不同", { providerId: "azure", model: "big-pickle" }]
  ])("%s时不是同一选择", (_case, other) => {
    expect(isSameModelSelection(bigPickle, other)).toBe(false);
  });

  it("任一侧为空时不是同一选择", () => {
    expect(isSameModelSelection(null, bigPickle)).toBe(false);
    expect(isSameModelSelection(bigPickle, undefined)).toBe(false);
    expect(isSameModelSelection(null, null)).toBe(false);
  });
});

describe("resolveModelSelect", () => {
  it("空闲时立即应用新选择", () => {
    expect(resolveModelSelect(false, bigPickle, deepseek)).toEqual({
      kind: "apply",
      selection: deepseek
    });
  });

  it("运行中点选新模型时暂存为待生效，不打断当前 turn", () => {
    expect(resolveModelSelect(true, bigPickle, deepseek)).toEqual({
      kind: "stage",
      selection: deepseek
    });
  });

  it("运行中点回当前生效模型时撤销暂存", () => {
    expect(resolveModelSelect(true, bigPickle, { ...bigPickle })).toEqual({ kind: "unstage" });
  });

  it("运行中且当前无生效模型时同样暂存", () => {
    expect(resolveModelSelect(true, null, deepseek)).toEqual({
      kind: "stage",
      selection: deepseek
    });
  });

  it("空闲时点选相同模型仍按应用处理", () => {
    expect(resolveModelSelect(false, bigPickle, { ...bigPickle })).toEqual({
      kind: "apply",
      selection: { ...bigPickle }
    });
  });
});
