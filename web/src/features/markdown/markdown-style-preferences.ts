export type MarkdownTableBorderStyle = "horizontal" | "grid" | "none";
export type MarkdownTableDensity = "compact" | "comfortable" | "spacious";
export type MarkdownCodeFontSize = "small" | "medium" | "large";
export type MarkdownCodeTabSize = "2" | "4" | "8";
export type MarkdownCodeMaxHeight = "none" | "medium" | "tall";

export type MarkdownTableStylePreferences = {
  borderStyle: MarkdownTableBorderStyle;
  density: MarkdownTableDensity;
  fullWidth: boolean;
  stripedRows: boolean;
  headerBackground: boolean;
  wrapCells: boolean;
};

export type MarkdownCodeBlockStylePreferences = {
  lineNumbers: boolean;
  wrapLongLines: boolean;
  showLanguageLabel: boolean;
  showCopyButton: boolean;
  showBorder: boolean;
  fontSize: MarkdownCodeFontSize;
  tabSize: MarkdownCodeTabSize;
  maxHeight: MarkdownCodeMaxHeight;
};

export type MarkdownStylePreferences = {
  table: MarkdownTableStylePreferences;
  codeBlock: MarkdownCodeBlockStylePreferences;
};

export const MARKDOWN_STYLE_STORAGE_KEY = "sai.markdown-style.v1";

export const DEFAULT_MARKDOWN_STYLE_PREFERENCES: MarkdownStylePreferences = {
  table: {
    borderStyle: "horizontal",
    density: "comfortable",
    fullWidth: true,
    stripedRows: false,
    headerBackground: false,
    wrapCells: true
  },
  codeBlock: {
    lineNumbers: false,
    wrapLongLines: false,
    showLanguageLabel: true,
    showCopyButton: true,
    showBorder: false,
    fontSize: "medium",
    tabSize: "2",
    maxHeight: "none"
  }
};

const TABLE_BORDER_STYLES: readonly MarkdownTableBorderStyle[] = ["horizontal", "grid", "none"];
const TABLE_DENSITIES: readonly MarkdownTableDensity[] = ["compact", "comfortable", "spacious"];
const CODE_FONT_SIZES: readonly MarkdownCodeFontSize[] = ["small", "medium", "large"];
const CODE_TAB_SIZES: readonly MarkdownCodeTabSize[] = ["2", "4", "8"];
const CODE_MAX_HEIGHTS: readonly MarkdownCodeMaxHeight[] = ["none", "medium", "tall"];

/**
 * 解析本地保存的 Markdown 外观配置，并为缺失或非法字段补齐默认值。
 *
 * @param raw 本地存储中的 JSON 文本
 * @returns 可直接用于渲染的完整配置
 */
export function parseMarkdownStylePreferences(raw: string | null | undefined): MarkdownStylePreferences {
  if (!raw) return cloneDefaultPreferences();
  try {
    return normalizeMarkdownStylePreferences(JSON.parse(raw) as unknown);
  } catch {
    return cloneDefaultPreferences();
  }
}

/**
 * 将未知对象归一化为完整的 Markdown 外观配置。
 *
 * @param value 待校验的配置对象
 * @returns 已校验并补齐默认值的配置
 */
export function normalizeMarkdownStylePreferences(value: unknown): MarkdownStylePreferences {
  const root = asRecord(value);
  const table = asRecord(root?.table);
  const codeBlock = asRecord(root?.codeBlock);
  const defaults = DEFAULT_MARKDOWN_STYLE_PREFERENCES;

  return {
    table: {
      borderStyle: readChoice(table?.borderStyle, TABLE_BORDER_STYLES, defaults.table.borderStyle),
      density: readChoice(table?.density, TABLE_DENSITIES, defaults.table.density),
      fullWidth: readBoolean(table?.fullWidth, defaults.table.fullWidth),
      stripedRows: readBoolean(table?.stripedRows, defaults.table.stripedRows),
      headerBackground: readBoolean(table?.headerBackground, defaults.table.headerBackground),
      wrapCells: readBoolean(table?.wrapCells, defaults.table.wrapCells)
    },
    codeBlock: {
      lineNumbers: readBoolean(codeBlock?.lineNumbers, defaults.codeBlock.lineNumbers),
      wrapLongLines: readBoolean(codeBlock?.wrapLongLines, defaults.codeBlock.wrapLongLines),
      showLanguageLabel: readBoolean(codeBlock?.showLanguageLabel, defaults.codeBlock.showLanguageLabel),
      showCopyButton: readBoolean(codeBlock?.showCopyButton, defaults.codeBlock.showCopyButton),
      showBorder: readBoolean(codeBlock?.showBorder, defaults.codeBlock.showBorder),
      fontSize: readChoice(codeBlock?.fontSize, CODE_FONT_SIZES, defaults.codeBlock.fontSize),
      tabSize: readChoice(codeBlock?.tabSize, CODE_TAB_SIZES, defaults.codeBlock.tabSize),
      maxHeight: readChoice(codeBlock?.maxHeight, CODE_MAX_HEIGHTS, defaults.codeBlock.maxHeight)
    }
  };
}

/**
 * 复制默认配置，避免调用方意外修改共享常量。
 *
 * @returns 新的默认配置对象
 */
function cloneDefaultPreferences(): MarkdownStylePreferences {
  return {
    table: { ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.table },
    codeBlock: { ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock }
  };
}

/**
 * 将未知值收窄为普通对象。
 *
 * @param value 待判断的值
 * @returns 普通对象；类型不匹配时返回 undefined
 */
function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

/**
 * 从允许值集合中读取字符串选项。
 *
 * @param value 待校验值
 * @param choices 允许值集合
 * @param fallback 非法值使用的默认值
 * @returns 合法选项或默认值
 */
function readChoice<T extends string>(value: unknown, choices: readonly T[], fallback: T): T {
  return typeof value === "string" && choices.includes(value as T) ? value as T : fallback;
}

/**
 * 读取布尔配置值。
 *
 * @param value 待校验值
 * @param fallback 非法值使用的默认值
 * @returns 布尔配置值
 */
function readBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}
