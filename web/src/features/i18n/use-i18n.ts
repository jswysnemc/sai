import { useContext } from "react";
import { I18nContext, type I18nContextValue } from "./i18n-context";

/**
 * 返回当前 Web 界面语言和翻译方法。
 *
 * @returns 国际化上下文
 */
export function useI18n(): I18nContextValue {
  return useContext(I18nContext);
}
