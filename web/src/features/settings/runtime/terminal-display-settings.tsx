import { useI18n } from "../../i18n/use-i18n";
import { DEFAULT_TERMINAL_PREFERENCES, useTerminalPreferences } from "../../terminal/terminal-preferences";
import { Button } from "../../../shared/ui/button/button";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SkNumberInput } from "../kit";

/**
 * 【终端设置】【显示偏好】即时调整当前浏览器中本地和 SSH 终端的显示。
 * @returns 终端字体、字号和回滚容量控件
 */
export function TerminalDisplaySettings() {
  const { t } = useI18n();
  const { preferences, update } = useTerminalPreferences();
  return <SettingsPanel title={t("Web terminal display", "网页终端显示")} description={t("Applies immediately to open terminals and stays in this browser. Reducing scrollback removes older displayed lines.", "即时应用到已打开的终端，仅保存在当前浏览器。减少回滚行数会移除较早的显示记录。")}
    actions={<Button size="small" onClick={() => update(DEFAULT_TERMINAL_PREFERENCES)}>{t("Reset", "恢复默认")}</Button>}>
    <FieldGrid columns={3}>
      <SettingsField label={t("Terminal font", "终端字体")} anchor="runtime.terminal.font" hint={t("Monospaced fallbacks preserve terminal character alignment.", "使用等宽字体回退，保持终端字符对齐。")}>
        <ChoicePills value={preferences.font} options={[{ value: "default", label: t("Default", "默认") }, { value: "system", label: t("System", "系统") }, { value: "monospace", label: t("Browser monospace", "浏览器等宽") }]} onChange={(font) => update({ font })} />
      </SettingsField>
      <SettingsField label={t("Font size", "终端字号")} anchor="runtime.terminal.font_size" size="sm">
        <SkNumberInput value={preferences.fontSizeRem} min={0.625} max={1.5} step={0.0625} unit="rem" onChange={(value) => update({ fontSizeRem: value ?? 0.75 })} />
      </SettingsField>
      <SettingsField label={t("Scrollback lines", "回滚行数")} anchor="runtime.terminal.scrollback" size="sm" hint={t("0 disables scrollback; maximum 50,000 lines.", "0 表示不保留回滚，最多 50000 行。")}>
        <SkNumberInput value={preferences.scrollback} min={0} max={50000} integer step={1000} onChange={(value) => update({ scrollback: value ?? 1000 })} />
      </SettingsField>
    </FieldGrid>
  </SettingsPanel>;
}
