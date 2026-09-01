import type { AppConfig, InputConfig, PasteImageKey, TerminalConfig } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";

/** 粘贴键位的可选项与说明。 */
const PASTE_KEY_OPTIONS: Array<{ value: PasteImageKey; en: string; zh: string }> = [
  { value: "ctrl_v", en: "Ctrl+V", zh: "Ctrl+V" },
  { value: "alt_v", en: "Alt+V", zh: "Alt+V" },
  { value: "both", en: "Both keys", zh: "两个键都响应" }
];

type TerminalSettingsFieldsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染网页终端 Shell 配置。
 *
 * @param props 应用配置与更新回调
 * @returns 网页终端配置字段
 */
export function TerminalSettingsFields({ config, onConfigChange }: TerminalSettingsFieldsProps) {
  const { t } = useI18n();
  const terminal: TerminalConfig = config.terminal ?? { shell: "" };
  const input: InputConfig = config.input ?? {};
  const pasteImageKey: PasteImageKey =
    PASTE_KEY_OPTIONS.find((option) => option.value === input.paste_image_key)?.value ?? "ctrl_v";

  return (
    <div className="settings-form-grid">
      <label className="settings-field full">
        <span>{t("Terminal Shell", "终端 Shell")}</span>
        <input
          type="text"
          value={terminal.shell}
          placeholder={t("Leave empty to use the platform default Shell", "留空使用平台默认 Shell")}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => onConfigChange({
            ...config,
            terminal: { ...terminal, shell: event.target.value }
          })}
        />
        <small>{t("Enter an executable path or name without startup arguments. Empty values use the login Shell on Unix and PowerShell on Windows.", "填写可执行文件路径或名称，不包含启动参数。Unix 留空使用用户登录 Shell，Windows 留空使用 PowerShell。")}</small>
      </label>

      <label className="settings-field full">
        <span>{t("TUI clipboard paste key", "TUI 剪贴板粘贴键")}</span>
        <select
          value={pasteImageKey}
          onChange={(event) => onConfigChange({
            ...config,
            input: { ...input, paste_image_key: event.target.value as PasteImageKey }
          })}
        >
          {PASTE_KEY_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {t(option.en, option.zh)}
            </option>
          ))}
        </select>
        <small>{t("Which key reads the system clipboard into the TUI input, including images. Windows terminals swallow Ctrl+V, so Alt+V is the default there.", "TUI 输入框用哪个键读取系统剪贴板（含图片）。Windows 终端会吞掉 Ctrl+V，因此 Windows 上默认是 Alt+V。")}</small>
      </label>
    </div>
  );
}
