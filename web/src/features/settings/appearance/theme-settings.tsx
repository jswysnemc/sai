import { Check } from "../../../shared/ui/icons";
import { useEffect, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { THEME_PRESETS, type ThemeId } from "../../theme/theme";
import { SettingsField, SettingsPanel } from "../kit";
import "../appearance-settings-section.css";

/**
 * 【Web 设置】【主题选择】显示紧凑主题色板并即时应用选中配色。
 * @param props 当前主题及切换回调
 * @returns 主题选择面板
 */
export function ThemeSettings({ theme, onThemeChange }: { theme: ThemeId; onThemeChange: (theme: ThemeId) => void }) {
  const { t } = useI18n();
  const [preview, setPreview] = useState<ThemeId | null>(null);
  useEffect(() => {
    // 1. 【外观设置】【悬停预览】只切换当前文档配色，选择后才由主题控制器保存
    document.documentElement.dataset.theme = preview ?? theme;
    return () => { document.documentElement.dataset.theme = theme; };
  }, [preview, theme]);
  return (
    <SettingsPanel title={t("Theme and colors", "主题与配色")} description={t("Hover or focus to preview; select to keep the theme.", "悬停或聚焦可预览，点击后保留所选主题。")}>
      <SettingsField label={t("Workspace theme", "工作区主题")} anchor="appearance.theme">
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-5">
          {THEME_PRESETS.map((preset) => (
            <Button
              className={preset.id === theme ? "theme-preset active" : "theme-preset"}
              onMouseEnter={() => setPreview(preset.id)}
              onMouseLeave={() => setPreview(null)}
              onFocus={() => setPreview(preset.id)}
              onBlur={() => setPreview(null)}
              onClick={() => { setPreview(null); onThemeChange(preset.id); }}
              aria-pressed={preset.id === theme}
              key={preset.id}
            >
              <span className="theme-swatches">
                {preset.colors.map((color) => <i style={{ background: color }} key={color} />)}
              </span>
              <strong>{t(preset.nameEn, preset.nameZh)}</strong>
              <small>{t(preset.descriptionEn, preset.descriptionZh)}</small>
              <Check size={14} className="theme-preset-check" />
            </Button>
          ))}
        </div>
      </SettingsField>
    </SettingsPanel>
  );
}
