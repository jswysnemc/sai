import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, SettingsField, SettingsPanel } from "../kit";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【工具方式】【默认配置】配置普通调用与脚本编排的组合，保留权限和脚本预算。
 * @param props 当前配置与草稿更新回调
 * @returns 使用统一控件的工具方式设置
 */
export function CodeModeSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  return <SettingsPanel title={t("Tool calling", "工具调用方式")} description={t(
    "Native calls and scripts share tool definitions, loaded tools and permissions.",
    "普通调用与脚本共用工具说明、加载状态和权限。"
  )}>
    <SettingsField label={t("Default tool mode", "默认工具方式")} anchor="runtime.codemode.mode" configKey="codemode.mode" hint={t(
      "Applies to new TUI, CLI and Web runs. Explicit session choices take precedence.",
      "用于新启动的终端、命令行和 Web 任务；显式会话设置优先。"
    )}>
      <ChoicePills value={config.codemode?.mode ?? "on"} options={[
        { value: "on" as const, label: t("Native + scripts", "普通与脚本并存") },
        { value: "off" as const, label: t("Native only", "仅普通工具") },
        { value: "only" as const, label: t("Scripts only", "仅脚本执行") }
      ]} onChange={(mode) => onConfigChange({ ...config, codemode: { ...config.codemode, mode } })} />
    </SettingsField>
  </SettingsPanel>;
}
