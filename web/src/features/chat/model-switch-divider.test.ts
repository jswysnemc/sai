import { describe, expect, it } from "vitest";
import { deriveModelSwitchMarkers } from "./model-switch-divider";

describe("deriveModelSwitchMarkers", () => {
  it("相邻轮次模型变化时在后一轮标注切换", () => {
    const markers = deriveModelSwitchMarkers([
      { key: "turn-1", model: "big-pickle" },
      { key: "turn-2", model: "big-pickle" },
      { key: "turn-3", model: "deepseek-v4-flash" }
    ]);

    expect(markers.size).toBe(1);
    expect(markers.get("turn-3")).toEqual({ from: "big-pickle", to: "deepseek-v4-flash" });
  });

  it("模型未变化时不产生任何标记", () => {
    const markers = deriveModelSwitchMarkers([
      { key: "turn-1", model: "big-pickle" },
      { key: "turn-2", model: "big-pickle" }
    ]);

    expect(markers.size).toBe(0);
  });

  it("首轮不标注切换，即使记录了模型", () => {
    expect(deriveModelSwitchMarkers([{ key: "turn-1", model: "big-pickle" }]).size).toBe(0);
  });

  it("未记录模型的旧轮次不参与对比也不中断前后比较", () => {
    const markers = deriveModelSwitchMarkers([
      { key: "turn-1", model: "big-pickle" },
      { key: "turn-2", model: null },
      { key: "turn-3" },
      { key: "turn-4", model: "big-pickle" },
      { key: "turn-5", model: "deepseek-v4-flash" }
    ]);

    expect(markers.size).toBe(1);
    expect(markers.get("turn-5")).toEqual({ from: "big-pickle", to: "deepseek-v4-flash" });
  });

  it("多次切换时每个变化点都有标记", () => {
    const markers = deriveModelSwitchMarkers([
      { key: "turn-1", model: "a" },
      { key: "turn-2", model: "b" },
      { key: "turn-3", model: "a" }
    ]);

    expect(markers.get("turn-2")).toEqual({ from: "a", to: "b" });
    expect(markers.get("turn-3")).toEqual({ from: "b", to: "a" });
  });

  it("空白模型串视为未记录", () => {
    const markers = deriveModelSwitchMarkers([
      { key: "turn-1", model: "big-pickle" },
      { key: "turn-2", model: "  " },
      { key: "turn-3", model: "deepseek-v4-flash" }
    ]);

    expect(markers.size).toBe(1);
    expect(markers.get("turn-3")).toEqual({ from: "big-pickle", to: "deepseek-v4-flash" });
  });

  it("历史轮次与实时运行混排时跨来源对比", () => {
    const markers = deriveModelSwitchMarkers([
      { key: "turn-1", model: "big-pickle" },
      { key: "run-live", model: "deepseek-v4-flash" }
    ]);

    expect(markers.get("run-live")).toEqual({ from: "big-pickle", to: "deepseek-v4-flash" });
  });
});
