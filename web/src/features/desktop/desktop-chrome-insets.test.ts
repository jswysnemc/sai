import { describe, expect, it } from "vitest";
import { rowInset } from "./desktop-chrome-insets";

const WINDOWS = { right: 138, left: 0, height: 32 };
const MAC = { right: 0, left: 78, height: 32 };

describe("desktop chrome row insets", () => {
  it("贴右上角的顶行让出窗口按钮宽度", () => {
    expect(rowInset({ left: 300, right: 1280, top: 0, bottom: 32 }, 1280, WINDOWS)).toEqual({ left: 0, right: 138 });
  });

  it("离窗口右缘较远的顶行不需要让位", () => {
    expect(rowInset({ left: 0, right: 248, top: 0, bottom: 32 }, 1280, WINDOWS)).toEqual({ left: 0, right: 0 });
  });

  it("部分伸入按钮区域时只让出重叠部分", () => {
    expect(rowInset({ left: 0, right: 1200, top: 0, bottom: 32 }, 1280, WINDOWS)).toEqual({ left: 0, right: 58 });
  });

  it("不在窗口顶端的行不受影响", () => {
    expect(rowInset({ left: 300, right: 1280, top: 300, bottom: 332 }, 1280, WINDOWS)).toEqual({ left: 0, right: 0 });
  });

  it("macOS 贴左上角的顶行为红绿灯让位", () => {
    expect(rowInset({ left: 10, right: 238, top: 8, bottom: 36 }, 1280, MAC)).toEqual({ left: 68, right: 0 });
    expect(rowInset({ left: 248, right: 1280, top: 0, bottom: 32 }, 1280, MAC)).toEqual({ left: 0, right: 0 });
  });
});
