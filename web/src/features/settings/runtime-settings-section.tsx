import type { AppConfig } from "../../api/contracts";
import { SettingsGroup } from "./editor-layout";
import { StructuredConfigFields } from "./structured-config-fields";
import { PermissionDefaultSettings } from "./runtime/permission-default-settings";
import { TerminalSettingsFields } from "./terminal-settings-fields";
import { CompactionModelField } from "./compaction-model-field";

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
  const groups = [
    ["tools", "工具执行", "控制工具轮次、Shell 和后台命令。"],
    ["skills", "技能系统", "控制技能加载和命令执行权限。"],
    ["display", "输出显示", "控制思考、工具调用和等待状态。"]
  ] as const;
  return (
    <div className="runtime-groups">
      <PermissionDefaultSettings config={config} onConfigChange={onConfigChange} />
      <SettingsGroup title="网页终端" description="配置网页终端启动的 Shell，新建终端时生效。">
        <TerminalSettingsFields config={config} onConfigChange={onConfigChange} />
      </SettingsGroup>
      <SettingsGroup title="上下文管理" description="上下文达到 90% 时自动压缩，也可以随时手动触发。">
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>默认上下文 token 数</span>
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
            <small>仅在模型没有单独配置上下文窗口时使用</small>
          </label>
          <CompactionModelField config={config} onConfigChange={onConfigChange} />
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
