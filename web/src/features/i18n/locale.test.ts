import { describe, expect, it } from "vitest";
import { detectInitialLocale, LOCALE_STORAGE_KEY, normalizeLocale, text } from "./locale";

describe("locale", () => {
  it("规范化常用中英文语言代码", () => {
    expect(normalizeLocale("en_GB")).toBe("en-US");
    expect(normalizeLocale("zh-TW")).toBe("zh-CN");
    expect(normalizeLocale("ja-JP")).toBeNull();
  });

  it("优先采用浏览器中已保存的语言", () => {
    const storage = { getItem: (key: string) => key === LOCALE_STORAGE_KEY ? "zh-CN" : null };
    expect(detectInitialLocale(storage, ["en-US"])).toBe("zh-CN");
  });

  it("没有支持的浏览器语言时回退到英文", () => {
    expect(detectInitialLocale({ getItem: () => null }, ["ja-JP"])).toBe("en-US");
  });

  it("按语言选择双语文本", () => {
    expect(text("en-US", "Settings", "设置")).toBe("Settings");
    expect(text("zh-CN", "Settings", "设置")).toBe("设置");
  });
});
