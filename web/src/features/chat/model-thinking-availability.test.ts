import { describe, expect, it } from "vitest";
import type { AppConfig, ThinkingLevel } from "../../api/contracts";
import type { ChatModelChoice } from "./chat-model-options";
import { modelThinkingLevels, resolveThinkingLevel } from "./model-thinking-availability";

/**
 * 构造只含一个供应商与模型元数据的配置。
 *
 * @param levels 该模型记录的支持等级
 * @returns 应用配置
 */
function config(levels?: string[]): AppConfig {
  return {
    active_provider: "p1",
    providers: [{
      id: "p1",
      display_name: "P1",
      base_url: "https://example.test/v1",
      models: ["m1"],
      default_model: "m1",
      ...(levels ? { model_metadata: { m1: { thinking_levels: levels } } } : {})
    }],
    gateways: {}
  } as unknown as AppConfig;
}

/** 当前选中的模型。 */
const SELECTION: ChatModelChoice = { providerId: "p1", providerName: "P1", model: "m1" };

describe("modelThinkingLevels", () => {
  it("在支持的等级前补上恒可用的 auto", () => {
    expect(modelThinkingLevels(config(["high", "max"]), SELECTION))
      .toEqual(["auto", "high", "max"]);
  });

  it("按强度升序返回，与配置里的顺序无关", () => {
    expect(modelThinkingLevels(config(["max", "low", "high"]), SELECTION))
      .toEqual(["auto", "low", "high", "max"]);
  });

  it("未记录支持范围时返回 undefined 表示不限制", () => {
    expect(modelThinkingLevels(config(), SELECTION)).toBeUndefined();
  });

  it("空列表同样视为未记录", () => {
    expect(modelThinkingLevels(config([]), SELECTION)).toBeUndefined();
  });

  it("全部是无法识别的值时视为未记录", () => {
    expect(modelThinkingLevels(config(["turbo"]), SELECTION)).toBeUndefined();
  });

  it("未选中模型时返回 undefined", () => {
    expect(modelThinkingLevels(config(["high"]), null)).toBeUndefined();
  });
});

describe("resolveThinkingLevel", () => {
  const available: ThinkingLevel[] = ["auto", "high", "max"];

  it("可用等级原样保留", () => {
    expect(resolveThinkingLevel(available, "max")).toBe("max");
  });

  it("超出范围的等级降到不超过它的最强档", () => {
    expect(resolveThinkingLevel(["auto", "low", "high"], "xhigh")).toBe("high");
  });

  it("请求低于全部可用档位时取其中最弱的", () => {
    expect(resolveThinkingLevel(available, "low")).toBe("high");
  });

  it("auto 不被降级", () => {
    expect(resolveThinkingLevel(available, "auto")).toBe("auto");
  });

  it("未记录支持范围时不改写请求等级", () => {
    expect(resolveThinkingLevel(undefined, "xhigh")).toBe("xhigh");
  });
});
