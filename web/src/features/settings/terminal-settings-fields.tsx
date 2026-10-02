import type { AppConfig, TerminalConfig } from "../../api/contracts";
import { SettingsField, SkTextInput, ChoicePills, Switch } from "./kit";
import { useI18n } from "../i18n/use-i18n";

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

  return (
    <>
    <SettingsField label={t("TUI rendering", "TUI 渲染方式")} configKey="display.fullscreen" anchor="runtime.display.fullscreen" hint={t("Used on the next TUI start. Ctrl+O toggles the current session.", "下次启动 TUI 时生效，Ctrl+O 可临时切换当前会话。")}>
      <ChoicePills value={config.display?.fullscreen === false ? "inline" : "global"} options={[{ value: "global", label: t("Global (default)", "全局渲染（默认）") }, { value: "inline", label: t("Inline", "行内渲染") }]} onChange={(value) => onConfigChange({ ...config, display: { ...config.display, fullscreen: value === "global" } })} />
    </SettingsField>
    {(["math_images", "mermaid_images"] as const).map((key) => (
      <SettingsField key={key} label={key === "math_images" ? t("Formula images in TUI", "TUI 公式图片") : t("Mermaid images in TUI", "TUI Mermaid 图片")} configKey={`display.${key}`} hint={t("Disable to show source text. Takes effect on the next TUI start.", "关闭后显示源码，下次启动 TUI 时生效。")}>
        <Switch ariaLabel={key === "math_images" ? t("Formula images in TUI", "TUI 公式图片") : t("Mermaid images in TUI", "TUI Mermaid 图片")} checked={config.display?.[key] !== false} onChange={(checked) => onConfigChange({ ...config, display: { ...config.display, [key]: checked } })} />
      </SettingsField>
    ))}
    <SettingsField
      label={t("Terminal Shell", "终端 Shell")}
      hint={t("Enter an executable path or name without startup arguments. Empty values use the login Shell on Unix and PowerShell on Windows.", "填写可执行文件路径或名称，不包含启动参数。Unix 留空使用用户登录 Shell，Windows 留空使用 PowerShell。")}
      configKey="terminal.shell"
      anchor="runtime.terminal.shell"
    >
      <SkTextInput mono value={terminal.shell} placeholder={t("Platform default Shell", "平台默认 Shell")} onChange={(value) => onConfigChange({ ...config, terminal: { ...terminal, shell: value } })} />
    </SettingsField>
    </>
  );
}
