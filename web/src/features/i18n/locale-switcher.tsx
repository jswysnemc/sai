import { Languages } from "lucide-react";
import type { Locale } from "./locale";
import { useI18n } from "./use-i18n";
import "./locale-switcher.css";

type LocaleSwitcherProps = {
  /** 折叠侧栏时使用仅图标样式 */
  compact?: boolean;
};

/**
 * 在中英文之间切换 Web 界面语言。
 *
 * @param props 展示形态
 * @returns 语言切换按钮
 */
export function LocaleSwitcher({ compact = false }: LocaleSwitcherProps) {
  const { locale, setLocale, t } = useI18n();
  const next: Locale = locale === "zh-CN" ? "en-US" : "zh-CN";
  const label = locale === "zh-CN" ? "中" : "EN";
  const title = t("Switch language", "切换语言");

  return (
    <button
      type="button"
      className={compact ? "sidebar-rail-button locale-switcher compact" : "locale-switcher"}
      onClick={() => setLocale(next)}
      title={`${title} · ${label}`}
      aria-label={title}
    >
      <Languages size={compact ? 17 : 15} strokeWidth={1.8} />
      {!compact && <span>{label}</span>}
    </button>
  );
}
