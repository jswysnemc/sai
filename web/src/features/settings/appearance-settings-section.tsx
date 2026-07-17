import { Check } from "lucide-react";
import { EditorHeader } from "./editor-layout";
import type { ThemeId } from "../theme/theme";
import { THEME_PRESETS } from "../theme/theme";

type AppearanceSettingsSectionProps = {
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
};

/**
 * 渲染主题配色选择区域。
 *
 * @param props 当前主题和更新回调
 * @returns 外观设置区域
 */
export function AppearanceSettingsSection({ theme, onThemeChange }: AppearanceSettingsSectionProps) {
  return (
    <section className="settings-editor">
      <EditorHeader kicker="界面外观" title="主题与配色" description="主题即时应用并保存在当前浏览器，不修改服务端配置。" />
      <div className="theme-preset-grid">
        {THEME_PRESETS.map((preset) => (
          <button
            type="button"
            className={preset.id === theme ? "theme-preset active" : "theme-preset"}
            onClick={() => onThemeChange(preset.id)}
            aria-pressed={preset.id === theme}
            key={preset.id}
          >
            <span className="theme-swatches">
              {preset.colors.map((color) => <i style={{ background: color }} key={color} />)}
            </span>
            <span className="theme-preset-copy"><strong>{preset.name}</strong><small>{preset.description}</small></span>
            <Check size={15} className="theme-preset-check" />
          </button>
        ))}
      </div>
    </section>
  );
}
