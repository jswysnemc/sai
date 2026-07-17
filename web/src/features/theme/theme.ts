import { useEffect, useState } from "react";

export type ThemeId = "system" | "linen" | "graphite" | "ocean";

export const THEME_PRESETS: Array<{ id: ThemeId; nameEn: string; nameZh: string; descriptionEn: string; descriptionZh: string; colors: string[] }> = [
  { id: "system", nameEn: "System", nameZh: "跟随系统", descriptionEn: "Match the operating system appearance", descriptionZh: "自动匹配系统明暗外观", colors: ["#f3f5f5", "#202526", "#477d70"] },
  { id: "linen", nameEn: "Linen", nameZh: "雾白", descriptionEn: "Low-contrast cool gray workspace", descriptionZh: "低对比冷灰专业界面", colors: ["#f3f5f5", "#202526", "#477d70"] },
  { id: "graphite", nameEn: "Graphite", nameZh: "石墨", descriptionEn: "Neutral dark engineering workspace", descriptionZh: "中性深色工程工作区", colors: ["#151a17", "#e5e9e6", "#52c488"] },
  { id: "ocean", nameEn: "Ocean", nameZh: "深海", descriptionEn: "Cool high-contrast workspace", descriptionZh: "冷色高辨识度工作区", colors: ["#101923", "#e5edf4", "#59b7d3"] }
];

const THEME_STORAGE_KEY = "sai.theme";
const THEME_IDS = THEME_PRESETS.map((preset) => preset.id);

/**
 * 在 React 渲染前应用已保存主题。
 *
 * @returns 当前主题标识
 */
export function initializeTheme(): ThemeId {
  const theme = loadTheme();
  document.documentElement.dataset.theme = theme;
  return theme;
}

/**
 * 管理当前界面主题和本地偏好。
 *
 * @returns 当前主题和更新方法
 */
export function useTheme() {
  const [theme, setTheme] = useState<ThemeId>(loadTheme);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  return { theme, setTheme };
}

/**
 * 读取合法的本地主题标识。
 *
 * @returns 主题标识
 */
function loadTheme(): ThemeId {
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY) as ThemeId | null;
  return stored && THEME_IDS.includes(stored) ? stored : "system";
}
