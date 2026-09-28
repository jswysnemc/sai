import type { AppConfig, TerminalConfig } from "../../api/contracts";
import { SettingsField, SkTextInput } from "./kit";
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
    <SettingsField
      label={t("Terminal Shell", "终端 Shell")}
      hint={t("Enter an executable path or name without startup arguments. Empty values use the login Shell on Unix and PowerShell on Windows.", "填写可执行文件路径或名称，不包含启动参数。Unix 留空使用用户登录 Shell，Windows 留空使用 PowerShell。")}
      configKey="terminal.shell"
      anchor="runtime.terminal.shell"
    >
      <SkTextInput mono value={terminal.shell} placeholder={t("Platform default Shell", "平台默认 Shell")} onChange={(value) => onConfigChange({ ...config, terminal: { ...terminal, shell: value } })} />
    </SettingsField>
  );
}
