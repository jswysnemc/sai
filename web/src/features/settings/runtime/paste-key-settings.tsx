import type { AppConfig, InputConfig, PasteImageKey } from "../../../api/contracts";
import { ChoicePills, SettingsField } from "../kit";
import { useI18n } from "../../i18n/use-i18n";

/** 粘贴键位的可选项与说明。 */
const PASTE_KEY_OPTIONS: Array<{ value: PasteImageKey; en: string; zh: string }> = [
  { value: "ctrl_v", en: "Ctrl+V", zh: "Ctrl+V" },
  { value: "alt_v", en: "Alt+V", zh: "Alt+V" },
  { value: "both", en: "Both keys", zh: "两个键都响应" }
];

/** 配置缺失时的回退键位；Windows 终端吞掉 Ctrl+V，与后端默认一致。 */
const DEFAULT_PASTE_KEY: PasteImageKey =
  typeof navigator !== "undefined" && /win/i.test(navigator.platform || navigator.userAgent)
    ? "alt_v"
    : "ctrl_v";

type PasteKeySettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * TUI 输入框粘贴键位设置。
 *
 * 键位属于 TUI 输入行为，不属于网页终端：分组与网页终端分开，
 * 用户才不会在网页终端设置里误改 TUI 的键。
 *
 * @param props 应用配置与更新回调
 * @returns 粘贴键位设置字段
 */
export function PasteKeySettings({ config, onConfigChange }: PasteKeySettingsProps) {
  const { t } = useI18n();
  const input: InputConfig = config.input ?? {};
  const pasteImageKey: PasteImageKey =
    PASTE_KEY_OPTIONS.find((option) => option.value === input.paste_image_key)?.value
    ?? DEFAULT_PASTE_KEY;

  return (
    <SettingsField label={t("TUI clipboard paste key", "TUI 剪贴板粘贴键")} configKey="input.paste_image_key" anchor="runtime.input.paste_image_key" hint={t("Reads text and images into the terminal UI. Windows defaults to Alt+V because terminals intercept Ctrl+V.", "将文字和图片粘贴到终端界面。Windows 终端会拦截 Ctrl+V，因此默认使用 Alt+V。")}>

      <ChoicePills
        value={pasteImageKey}
        options={PASTE_KEY_OPTIONS.map((option) => ({
          value: option.value,
          label: t(option.en, option.zh)
        }))}
        onChange={(value) => onConfigChange({
          ...config,
          input: { ...input, paste_image_key: value }
        })}
      />
    </SettingsField>
  );
}
