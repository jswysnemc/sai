import type { ITerminalOptions, ITheme } from "@xterm/xterm";

export const TERMINAL_FONT_FAMILY = '"Fira Code", "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace';

/**
 * 创建终端显示和交互选项。
 *
 * @returns xterm 终端选项
 */
export function createTerminalOptions(): ITerminalOptions {
  return {
    fontFamily: TERMINAL_FONT_FAMILY,
    fontSize: 12,
    fontWeight: "400",
    fontWeightBold: "500",
    letterSpacing: 0,
    lineHeight: 1.3,
    cursorBlink: true,
    theme: createTerminalTheme()
  };
}

/**
 * 从 CSS 主题令牌读取终端配色，保证终端背景与代码块、工具视图一致。
 *
 * @returns xterm 颜色主题
 */
export function createTerminalTheme(): ITheme {
  const tokens = getComputedStyle(document.documentElement);
  // 1. 读取统一技术表面令牌，缺失时回退到内置深色值
  const read = (name: string, fallback: string) => tokens.getPropertyValue(name).trim() || fallback;
  const background = read("--terminal-surface", "#101412");
  // 2. 按背景亮度选择 ANSI 调色板，保证浅色背景下黄色、亮色可读
  const palette = isLightColor(background) ? LIGHT_ANSI : DARK_ANSI;
  return {
    background,
    foreground: read("--terminal-text", "#e5e9e6"),
    cursor: read("--signal", "#9cbfb5"),
    selectionBackground: read("--terminal-selection", "#2b3934"),
    ...palette
  };
}

/** 浅色背景下的 ANSI 颜色。 */
const LIGHT_ANSI: Partial<ITheme> = {
  black: "#3b4245",
  red: "#b33a32",
  green: "#2f7d4c",
  yellow: "#9c6a1d",
  blue: "#2f668c",
  magenta: "#8a4d9e",
  cyan: "#2a7f83",
  white: "#c7cdcd",
  brightBlack: "#6f797b",
  brightRed: "#c94b42",
  brightGreen: "#38935b",
  brightYellow: "#b07b23",
  brightBlue: "#3a7aa6",
  brightMagenta: "#a05fb6",
  brightCyan: "#329499",
  brightWhite: "#202526"
};

/** 深色背景下的 ANSI 颜色。 */
const DARK_ANSI: Partial<ITheme> = {
  black: "#2c3431",
  red: "#e4938d",
  green: "#9bc585",
  yellow: "#d5b587",
  blue: "#8fb7d7",
  magenta: "#c8a3d8",
  cyan: "#8fd0c6",
  white: "#c8d2cc",
  brightBlack: "#69766f",
  brightRed: "#ef9189",
  brightGreen: "#a9d894",
  brightYellow: "#e2c391",
  brightBlue: "#9dc4e2",
  brightMagenta: "#d5b1e4",
  brightCyan: "#9adcd2",
  brightWhite: "#eef2ef"
};

/**
 * 判断颜色是否为浅色。
 *
 * @param color 十六进制颜色
 * @returns 亮度超过阈值时为 true
 */
function isLightColor(color: string): boolean {
  const match = /^#([0-9a-f]{6})$/i.exec(color.trim());
  if (!match) return false;
  const value = Number.parseInt(match[1], 16);
  // 1. 按感知亮度加权 RGB 分量
  const luminance = 0.299 * ((value >> 16) & 255) + 0.587 * ((value >> 8) & 255) + 0.114 * (value & 255);
  return luminance > 150;
}
