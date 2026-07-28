import { describe, expect, test } from "vitest";
import { isDarkTheme, THEME_PRESETS } from "./theme";

describe("theme presets", () => {
  /**
   * 校验主题目录中的标识唯一，且每套主题都有完整预览色。
   *
   * @returns 无
   */
  test("keeps unique ids and complete preview colors", () => {
    const ids = THEME_PRESETS.map((preset) => preset.id);

    expect(THEME_PRESETS).toHaveLength(10);
    expect(new Set(ids).size).toBe(ids.length);
    expect(THEME_PRESETS.every((preset) => preset.colors.length === 3)).toBe(true);
  });

  /**
   * 校验新增亮色与暗色主题使用正确的外观分类。
   *
   * @returns 无
   */
  test("classifies sage and ember appearances", () => {
    expect(isDarkTheme("sage")).toBe(false);
    expect(isDarkTheme("ember")).toBe(true);
  });
});
