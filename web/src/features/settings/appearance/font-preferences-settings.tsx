import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import {
  DEFAULT_EDITOR_FONT_SIZE,
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_SIZE_MIN,
  useFontPreferences
} from "../../theme/font-preferences";
import { SettingsField, SettingsPanel, SkNumberInput } from "../kit";
import { useLocalFontFamilies } from "../../theme/use-local-font-families";
import { FontFamilyPicker } from "./font-family-picker";
import "./font-preferences-settings.css";

/**
 * 【Web 设置】【字体】分别设置界面字族和编辑器字族，编辑器字号可在编辑器内用 Ctrl+滚轮调整。
 *
 * @returns 字体设置面板
 */
export function FontPreferencesSettings() {
  const { t } = useI18n();
  const { preferences, setPreferences, resetPreferences } = useFontPreferences();
  const fonts = useLocalFontFamilies();
  const requestFonts = () => {
    if (fonts.families.length === 0 && fonts.status !== "loading") void fonts.reload();
  };

  return (
    <SettingsPanel
      title={t("Fonts", "字体")}
      description={t(
        "Choose an installed font for each role. Chinese defaults to a monospace family. Hold Ctrl and scroll inside an editor to change its size.",
        "三项都从本机已安装字体里选择。中文默认使用等宽字族。在编辑器内按住 Ctrl 并滚动滚轮可调整字号。"
      )}
      actions={<Button size="small" onClick={resetPreferences}>{t("Reset", "恢复默认")}</Button>}
    >
      <SettingsField
        label={t("Interface font", "界面字体")}
        anchor="appearance.uiFont"
        hint={t("Leave the default to use Inter.", "选默认时使用 Inter。")}
      >
        <FontFamilyPicker
          value={preferences.uiFont}
          families={fonts.families}
          defaultLabel={t("Default · Inter", "默认 · Inter")}
          loaded={fonts.status === "ready"}
          onOpen={requestFonts}
          onChange={(uiFont) => setPreferences({ uiFont })}
        />
      </SettingsField>
      <SettingsField
        label={t("Editor font", "编辑器字体")}
        anchor="appearance.editorFont"
        hint={t("Leave the default to use Fira Code.", "选默认时使用 Fira Code。")}
      >
        <FontFamilyPicker
          value={preferences.editorFont}
          families={fonts.families}
          defaultLabel={t("Default · Fira Code", "默认 · Fira Code")}
          loaded={fonts.status === "ready"}
          onOpen={requestFonts}
          onChange={(editorFont) => setPreferences({ editorFont })}
        />
      </SettingsField>
      <SettingsField
        label={t("Chinese font", "中文字体")}
        anchor="appearance.cjkFont"
        hint={t(
          "Leave the default for Sarasa Mono SC, Noto Sans Mono CJK SC, then Source Han Mono.",
          "选默认时依次使用 Sarasa Mono SC、Noto Sans Mono CJK SC、思源等宽。"
        )}
      >
        <FontFamilyPicker
          value={preferences.cjkFont}
          families={fonts.families}
          defaultLabel={t("Default · Monospace", "默认 · 等宽")}
          loaded={fonts.status === "ready"}
          onOpen={requestFonts}
          onChange={(cjkFont) => setPreferences({ cjkFont })}
        />
      </SettingsField>
      {(fonts.status === "denied" || fonts.status === "idle" || (fonts.status === "ready" && fonts.families.length === 0)) && (
        <button type="button" className="font-family-load" onClick={() => void fonts.reload()}>
          {t("Load installed fonts", "读取本机字体")}
        </button>
      )}
      {fonts.status === "unavailable" && (
        <p className="font-family-load-note">{t("This browser cannot list installed fonts.", "当前浏览器不能列出本机字体。")}</p>
      )}
      <SettingsField
        label={t("Editor font size", "编辑器字号")}
        anchor="appearance.editorFontSize"
        hint={t(
          `${EDITOR_FONT_SIZE_MIN}–${EDITOR_FONT_SIZE_MAX} px. Default ${DEFAULT_EDITOR_FONT_SIZE}px. Ctrl and the wheel change this value.`,
          `${EDITOR_FONT_SIZE_MIN}–${EDITOR_FONT_SIZE_MAX} 像素，默认 ${DEFAULT_EDITOR_FONT_SIZE}px。Ctrl 加滚轮会改这里的数值。`
        )}
        size="sm"
      >
        <SkNumberInput
          value={preferences.editorFontSize}
          min={EDITOR_FONT_SIZE_MIN}
          max={EDITOR_FONT_SIZE_MAX}
          integer
          step={1}
          unit="px"
          ariaLabel={t("Editor font size", "编辑器字号")}
          onChange={(value) => {
            if (value != null) setPreferences({ editorFontSize: value });
          }}
        />
      </SettingsField>
      <section className="font-preferences-preview" aria-label={t("Font preview", "字体预览")}>
        <p className="font-preferences-preview-ui">{t("Interface text 界面文字", "界面文字 Aa")}</p>
        <p className="font-preferences-preview-editor">const 字号 = fontSize;</p>
      </section>
    </SettingsPanel>
  );
}
