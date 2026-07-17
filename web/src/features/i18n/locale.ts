export const LOCALE_STORAGE_KEY = "sai.locale";
export const SUPPORTED_LOCALES = ["en-US", "zh-CN"] as const;

export type Locale = (typeof SUPPORTED_LOCALES)[number];

type LocaleStorage = Pick<Storage, "getItem">;

/**
 * 将浏览器或用户输入的语言代码规范化为受支持语言。
 *
 * @param value 待解析语言代码
 * @returns 支持的语言；无法识别时返回空
 */
export function normalizeLocale(value: string | null | undefined): Locale | null {
  if (!value) return null;
  const normalized = value.trim().replaceAll("_", "-").toLowerCase();
  if (normalized === "en" || normalized.startsWith("en-")) return "en-US";
  if (normalized === "zh" || normalized.startsWith("zh-")) return "zh-CN";
  return null;
}

/**
 * 按本地偏好和浏览器语言检测初始界面语言。
 *
 * @param storage 可选本地存储读取器
 * @param browserLanguages 浏览器候选语言列表
 * @returns 初始界面语言
 */
export function detectInitialLocale(
  storage: LocaleStorage | null = typeof window === "undefined" ? null : window.localStorage,
  browserLanguages: readonly string[] = typeof navigator === "undefined" ? [] : navigator.languages
): Locale {
  const stored = normalizeLocale(storage?.getItem(LOCALE_STORAGE_KEY));
  if (stored) return stored;
  for (const language of browserLanguages) {
    const locale = normalizeLocale(language);
    if (locale) return locale;
  }
  return "en-US";
}

/**
 * 按指定语言选择界面文本。
 *
 * @param locale 当前语言
 * @param en 英文文本
 * @param zh 简体中文文本
 * @returns 与当前语言匹配的文本
 */
export function text(locale: Locale, en: string, zh: string): string {
  return locale === "zh-CN" ? zh : en;
}
