import { useEffect, useState } from "react";

export type ThemeId =
  | "system"
  | "linen"
  | "graphite"
  | "ocean"
  | "amber"
  | "dusk"
  | "slate"
  | "rose"
  | "sage"
  | "ember";

export const THEME_PRESETS: Array<{
  id: ThemeId;
  nameEn: string;
  nameZh: string;
  descriptionEn: string;
  descriptionZh: string;
  colors: string[];
  dark?: boolean;
}> = [
  {
    id: "system",
    nameEn: "System",
    nameZh: "跟随系统",
    descriptionEn: "Match the operating system appearance",
    descriptionZh: "自动匹配系统明暗外观",
    colors: ["#f3f5f5", "#202526", "#477d70"]
  },
  {
    id: "linen",
    nameEn: "Linen",
    nameZh: "雾白",
    descriptionEn: "Low-contrast cool gray workspace",
    descriptionZh: "低对比冷灰专业界面",
    colors: ["#f3f5f5", "#202526", "#477d70"]
  },
  {
    id: "amber",
    nameEn: "Amber",
    nameZh: "琥珀",
    descriptionEn: "Warm paper with copper accents",
    descriptionZh: "暖纸色与铜色强调",
    colors: ["#f7f1e8", "#2a241c", "#b7791f"]
  },
  {
    id: "slate",
    nameEn: "Slate",
    nameZh: "青石",
    descriptionEn: "Cool blue-gray daylight workspace",
    descriptionZh: "冷蓝灰日光工作区",
    colors: ["#eef2f6", "#1d2730", "#3d7ea6"]
  },
  {
    id: "rose",
    nameEn: "Rose",
    nameZh: "蔷薇",
    descriptionEn: "Soft rose light with berry signal",
    descriptionZh: "浅蔷薇底与浆果色信号",
    colors: ["#f8eef1", "#2a1f24", "#b45a78"]
  },
  {
    id: "sage",
    nameEn: "Sage",
    nameZh: "苔绿",
    descriptionEn: "Muted green-gray daylight workspace",
    descriptionZh: "低饱和灰绿日光工作区",
    colors: ["#edf2ec", "#1f2923", "#46705a"]
  },
  {
    id: "graphite",
    nameEn: "Graphite",
    nameZh: "石墨",
    descriptionEn: "Neutral dark engineering workspace",
    descriptionZh: "中性深色工程工作区",
    colors: ["#151a17", "#e5e9e6", "#52c488"],
    dark: true
  },
  {
    id: "ocean",
    nameEn: "Ocean",
    nameZh: "深海",
    descriptionEn: "Cool high-contrast workspace",
    descriptionZh: "冷色高辨识度工作区",
    colors: ["#101923", "#e5edf4", "#59b7d3"],
    dark: true
  },
  {
    id: "dusk",
    nameEn: "Dusk",
    nameZh: "暮色",
    descriptionEn: "Violet-night coding workspace",
    descriptionZh: "紫夜编码工作区",
    colors: ["#17141f", "#ebe6f4", "#a78bfa"],
    dark: true
  },
  {
    id: "ember",
    nameEn: "Ember",
    nameZh: "余烬",
    descriptionEn: "Warm charcoal with ember accents",
    descriptionZh: "暖炭灰与余烬橙强调",
    colors: ["#1b1715", "#f0e8e1", "#df965f"],
    dark: true
  }
];

const THEME_STORAGE_KEY = "sai.theme";
const THEME_IDS = THEME_PRESETS.map((preset) => preset.id);
const DARK_THEME_IDS = new Set(
  THEME_PRESETS.filter((preset) => preset.dark).map((preset) => preset.id)
);

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
 * 判断主题是否按深色界面处理（含系统跟随）。
 *
 * @param theme 主题标识
 * @returns 是否深色
 */
export function isDarkTheme(theme: ThemeId): boolean {
  if (DARK_THEME_IDS.has(theme)) return true;
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  }
  return false;
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
