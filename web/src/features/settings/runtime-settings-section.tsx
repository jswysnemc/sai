import type { AppConfig } from "../../api/contracts";
import { SettingsGroup } from "./editor-layout";
import { StructuredConfigFields } from "./structured-config-fields";
import { AgentEngineSettings } from "./runtime/agent-engine-settings";
import { PermissionDefaultSettings } from "./runtime/permission-default-settings";
import { NotificationSettings } from "./runtime/notification-settings";
import { TerminalSettingsFields } from "./terminal-settings-fields";
import { RtkFilterSettings } from "./rtk-filter-settings";
import { CompactionModelField } from "./compaction-model-field";
import { MemoryExtractionModelField } from "./memory-extraction-model-field";
import { useI18n } from "../i18n/use-i18n";

type RuntimeSettingsSectionProps = {
  config: AppConfig;
  /** 当前子页：engine / permissions / notifications / terminal / context / tools */
  subview?: string;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染运行时分区的当前子页。
 *
 * 原先八个配置组平铺一页难以定位，现按注册表子页拆开：
 * 每个子页只承载一个领域，路由刷新后停留在原子页。
 *
 * @param props 应用配置、当前子页与更新回调
 * @returns 运行时设置子页内容
 */
export function RuntimeSettingsSection({ config, subview, onConfigChange }: RuntimeSettingsSectionProps) {
  const { t } = useI18n();
  switch (subview) {
    case "permissions":
      return <PermissionDefaultSettings config={config} onConfigChange={onConfigChange} />;
    case "notifications":
      return <NotificationSettings config={config} onConfigChange={onConfigChange} />;
    case "terminal":
      return (
        <SettingsGroup
          title={t("Web terminal", "网页终端")}
          description={t("Configure the Shell used by new Web terminal sessions.", "配置网页终端启动的 Shell，新建终端时生效。")}
        >
          <TerminalSettingsFields config={config} onConfigChange={onConfigChange} />
        </SettingsGroup>
      );
    case "context":
      return (
        <SettingsGroup
          title={t("Context management", "上下文管理")}
          description={t(
            "Context compacts automatically at 90% capacity and can also be triggered manually.",
            "上下文达到 90% 时自动压缩，也可以随时手动触发。"
          )}
        >
          <div className="settings-form-grid">
            <label className="settings-field">
              <span>{t("Default context tokens", "默认上下文 token 数")}</span>
              <input
                type="number"
                min={1}
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
      );
    case "tools":
      return (
        <div className="runtime-groups">
          <SettingsGroup
            title={t("Tool execution", "工具执行")}
            description={t("Control tool rounds, Shell, and background commands.", "控制工具轮次、Shell 和后台命令。")}
          >
            <StructuredConfigFields
              value={withoutRtkFields((config.tools as Record<string, unknown> | undefined) ?? {})}
              onChange={(next) => onConfigChange({ ...config, tools: { ...(config.tools ?? {}), ...next } })}
            />
          </SettingsGroup>
          <SettingsGroup
            title={t("Command output filter (rtk)", "命令输出过滤器（rtk）")}
            description={t(
              "Route commands through rtk to compress their output and save context tokens. Excluded commands run unchanged.",
              "命令经由 rtk 执行以压缩输出、节省上下文 token。排除的命令保持原样执行。"
            )}
          >
            <RtkFilterSettings config={config} onConfigChange={onConfigChange} />
          </SettingsGroup>
          <SettingsGroup
            title={t("Output display", "输出显示")}
            description={t("Control reasoning, tool calls, and waiting status.", "控制思考、工具调用和等待状态。")}
          >
            <StructuredConfigFields
              value={(config.display as Record<string, unknown> | undefined) ?? {}}
              onChange={(next) => onConfigChange({ ...config, display: next })}
            />
          </SettingsGroup>
        </div>
      );
    case "engine":
    default:
      return (
        <SettingsGroup
          title={t("Conversation engine", "对话内核")}
          description={t(
            "Run the reasoning loop with sai's own engine or hand it to an external ACP agent.",
            "用 sai 自带内核执行推理循环，或交给外部 ACP agent。"
          )}
        >
          <AgentEngineSettings config={config} onConfigChange={onConfigChange} />
        </SettingsGroup>
      );
  }
}

/** rtk 过滤字段由专属配置组接管，通用工具字段中排除。 */
function withoutRtkFields(tools: Record<string, unknown>): Record<string, unknown> {
  const { command_filter: _filter, command_filter_denylist: _denylist, ...rest } = tools;
  return rest;
}
