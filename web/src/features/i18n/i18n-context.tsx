import { createContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { detectInitialLocale, LOCALE_STORAGE_KEY, text, type Locale } from "./locale";

export type Translate = (en: string, zh: string) => string;

export type I18nContextValue = {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: Translate;
};

const fallbackValue: I18nContextValue = {
  locale: "zh-CN",
  setLocale: () => undefined,
  t: (_en, zh) => zh
};

export const I18nContext = createContext<I18nContextValue>(fallbackValue);

/**
 * 提供 Web 界面语言状态，并同步浏览器持久化偏好和 HTML lang 属性。
 *
 * @param props 应用子节点
 * @returns 国际化上下文提供器
 */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocale] = useState<Locale>(detectInitialLocale);

  useEffect(() => {
    document.documentElement.lang = locale;
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  }, [locale]);

  const value = useMemo<I18nContextValue>(() => ({
    locale,
    setLocale,
    t: (en, zh) => text(locale, en, zh)
  }), [locale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
