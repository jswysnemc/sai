import type { AppConfig } from "../../api/contracts";
import { SettingsGroup } from "./editor-layout";
import { StructuredConfigFields } from "./structured-config-fields";
import { AgentEngineSettings } from "./runtime/agent-engine-settings";
import { PermissionDefaultSettings } from "./runtime/permission-default-settings";
import { NotificationSettings } from "./runtime/notification-settings";
import { TerminalSettingsFields } from "./terminal-settings-fields";
import { RtkFilterSettings } from "./rtk-filter-settings";
import { ToggleRow } from "./controls/toggle-row";
import { CompactionModelField } from "./compaction-model-field";
import { MemoryExtractionModelField } from "./memory-extraction-model-field";
import { useI18n } from "../i18n/use-i18n";
import { DebugSettings } from "./runtime/debug-settings";

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
            "Workspace defaults for new sessions. The chat context panel can override ratio and reserve for the current session.",
            "新会话的工作区默认值。对话里的上下文用量可为本会话单独改比例与预留。"
          )}
        >
          <div className="settings-form-grid">
            <label className="settings-field">
              <span>{t("Default context tokens", "默认上下文 token 数")}</span>
              <input
                type="number"
                min={1}
                value={config.context?.default_max_chars ?? 120_000}
                onChange={(event) => onConfigChange(updateContext(config, {
                  default_max_chars: Math.max(1, Number(event.target.value))
                }))}
              />
              <small>{t("Used only when the model has no dedicated context window setting", "仅在模型没有单独配置上下文窗口时使用")}</small>
            </label>
            <label className="settings-field">
              <span>{t("Auto-compact ratio", "自动压缩比例")}</span>
              <input
                type="number"
                min={50}
                max={99}
                step={1}
                value={Math.round((config.context?.compaction_ratio ?? 0.9) * 100)}
                onChange={(event) => onConfigChange(updateContext(config, {
                  compaction_ratio: clampCompactionRatio(Number(event.target.value) / 100)
                }))}
              />
              <small>{t("Percent of the session context window. 90 means compact at 90%.", "占当前会话上下文窗口的百分比。90 表示用到 90% 再压缩。")}</small>
            </label>
            <label className="settings-field">
              <span>{t("Reserved headroom", "压缩预留 token")}</span>
              <input
                type="number"
                min={0}
                step={1000}
                value={config.context?.compaction_reserve_tokens ?? 50_000}
                onChange={(event) => onConfigChange(updateContext(config, {
                  compaction_reserve_tokens: Math.max(0, Number(event.target.value) || 0)
                }))}
              />
              <small>{t("Large windows compact when this many tokens remain. 0 uses the ratio only. Small windows still follow the ratio.", "大窗口在剩余不足该值时压缩。0 表示只按比例。小窗口仍按比例，不会被预留拖早。")}</small>
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
            description={t("Control tool availability, Shell, and background commands.", "控制工具可用性、Shell 和后台命令。")}
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
          <SettingsGroup
            title={t("Session mesh", "会话网格")}
            description={t(
              "Mesh tools can deliver messages to other sessions and subagents. Cross-session delivery stays off by default.",
              "网格工具可向其他会话与子智能体投递消息。跨会话投递默认关闭。"
            )}
          >
            <ToggleRow
              label={t("Cross-session messaging", "跨会话投递")}
              hint={t("Send mesh messages to sessions other than this one", "向本会话以外的会话发送网格消息")}
              checked={config.mesh?.cross_session ?? false}
              onChange={(checked) => onConfigChange({ ...config, mesh: { cross_session: checked } })}
            />
          </SettingsGroup>
          <DebugSettings config={config} onConfigChange={onConfigChange} />
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

/**
 * 合并上下文管理字段，保留未编辑项。
 *
 * @param config 当前应用配置
 * @param patch 要覆盖的上下文字段
 * @returns 更新后的应用配置
 */
function updateContext(config: AppConfig, patch: Partial<NonNullable<AppConfig["context"]>>): AppConfig {
  return {
    ...config,
    context: {
      default_max_chars: 120_000,
      ...config.context,
      ...patch
    }
  };
}

/**
 * 把压缩比例限制在 50%–99%。
 *
 * @param ratio 0–1 比例
 * @returns 夹紧后的比例
 */
function clampCompactionRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) return 0.9;
  return Math.min(0.99, Math.max(0.5, ratio));
}

/** rtk 过滤字段由专属配置组接管；轮次上限已取消，通用工具字段中一并排除。 */
function withoutRtkFields(tools: Record<string, unknown>): Record<string, unknown> {
  const {
    command_filter: _filter,
    command_filter_denylist: _denylist,
    max_rounds: _maxRounds,
    ...rest
  } = tools;
  return rest;
}
