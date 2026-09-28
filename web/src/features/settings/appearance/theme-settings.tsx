import { Check } from "../../../shared/ui/icons";
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
  return (
    <SettingsPanel title={t("Theme and colors", "主题与配色")}>
      <SettingsField label={t("Workspace theme", "工作区主题")} anchor="appearance.theme">
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-5">
          {THEME_PRESETS.map((preset) => (
            <Button
              className={preset.id === theme ? "theme-preset active" : "theme-preset"}
              onClick={() => onThemeChange(preset.id)}
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
