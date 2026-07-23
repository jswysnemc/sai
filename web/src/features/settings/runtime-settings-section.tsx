import type { AppConfig } from "../../api/contracts";
import { SettingsGroup } from "./editor-layout";
import { StructuredConfigFields } from "./structured-config-fields";
import { PermissionDefaultSettings } from "./runtime/permission-default-settings";
import { NotificationSettings } from "./runtime/notification-settings";
import { TerminalSettingsFields } from "./terminal-settings-fields";
import { CompactionModelField } from "./compaction-model-field";
import { MemoryExtractionModelField } from "./memory-extraction-model-field";
import { useI18n } from "../i18n/use-i18n";

type RuntimeSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染工具、技能、显示和上下文运行参数。
 *
 * @param props 应用配置和更新回调
 * @returns 运行参数设置区域
 */
export function RuntimeSettingsSection({ config, onConfigChange }: RuntimeSettingsSectionProps) {
  const { t } = useI18n();
  const groups = [
    ["tools", t("Tool execution", "工具执行"), t("Control tool rounds, Shell, and background commands.", "控制工具轮次、Shell 和后台命令。")],
    ["skills", t("Skill system", "技能系统"), t("Control skill loading and command execution permissions.", "控制技能加载和命令执行权限。")],
    ["display", t("Output display", "输出显示"), t("Control reasoning, tool calls, and waiting status.", "控制思考、工具调用和等待状态。")]
  ] as const;
  return (
    <div className="runtime-groups">
      <PermissionDefaultSettings config={config} onConfigChange={onConfigChange} />
      <NotificationSettings config={config} onConfigChange={onConfigChange} />
      <SettingsGroup title={t("Web terminal", "网页终端")} description={t("Configure the Shell used by new Web terminal sessions.", "配置网页终端启动的 Shell，新建终端时生效。")}>
        <TerminalSettingsFields config={config} onConfigChange={onConfigChange} />
      </SettingsGroup>
      <SettingsGroup title={t("Context management", "上下文管理")} description={t("Context compacts automatically at 90% capacity and can also be triggered manually.", "上下文达到 90% 时自动压缩，也可以随时手动触发。")}>
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Default context tokens", "默认上下文 token 数")}</span>
            <input
              type="number"
              min="1"
              value={config.context?.default_max_chars ?? 120_000}
              onChange={(event) => onConfigChange({
                ...config,
                context: {
                  ...(config.context ?? { default_max_chars: 120_000 }),
                  default_max_chars: Math.max(1, Number(event.target.value))
                }
              })}
            />
            <small>{t("Used only when the model has no dedicated context window setting", "仅在模型没有单独配置上下文窗口时使用")}</small>
          </label>
          <CompactionModelField config={config} onConfigChange={onConfigChange} />
          <MemoryExtractionModelField config={config} onConfigChange={onConfigChange} />
        </div>
      </SettingsGroup>
      {groups.map(([key, title, description]) => (
        <SettingsGroup title={title} description={description} key={key}>
          <StructuredConfigFields
            value={(config[key] as Record<string, unknown> | undefined) ?? {}}
            onChange={(next) => onConfigChange({ ...config, [key]: next })}
          />
        </SettingsGroup>
      ))}
    </div>
  );
}
