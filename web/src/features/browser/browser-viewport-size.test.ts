import { describe, expect, it } from "vitest";
import { browserViewportSize } from "./browser-viewport-size";
import { toPagePoint } from "./browser-input";

describe("实际视口渲染", () => {
  it("窄面板直接使用实际宽高，不再强制 1280px 桌面宽度", () => {
    const host = { width: 703, height: 1084 };
    expect(browserViewportSize(host)).toEqual(host);
    expect(toPagePoint(351, 542, {
      left: 0, top: 0, ...host, pageWidth: host.width, pageHeight: host.height
    })).toEqual({ x: 351, y: 542 });
  });

  it("移动端和桌面端均独立同步宽高，不通过宽度推算高度", () => {
    for (const host of [{ width: 360, height: 740 }, { width: 1440, height: 900 }]) {
      expect(browserViewportSize(host)).toEqual(host);
    }
    expect(browserViewportSize({ width: 703.9, height: 1084.5 })).toEqual({ width: 703, height: 1084 });
  });

  it("保留服务端尺寸限制，隐藏或无效面板不发送尺寸", () => {
    expect(browserViewportSize({ width: 6000, height: 3000 })).toEqual({ width: 3840, height: 2160 });
    expect(browserViewportSize({ width: 100, height: 100 })).toEqual({ width: 320, height: 240 });
    expect(browserViewportSize({ width: 0, height: 800 })).toBeNull();
    expect(browserViewportSize({ width: NaN, height: 800 })).toBeNull();
  });
});
