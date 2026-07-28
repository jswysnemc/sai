import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { AgentEngineKind, AppConfig } from "../../../api/contracts";
import { Select, type SelectOption } from "../../../shared/ui/select/select";
import { AgentEngineBrandIcon } from "../../../shared/ui/agent-engine-brand-icon/agent-engine-brand-icon";
import { useI18n } from "../../i18n/use-i18n";
import { resetNewSessionEnginePreferences } from "../../sessions/new-session-preferences";
import { AcpCapabilityPanel } from "./acp-capability-panel";
import { AcpRuntimeConfigFields } from "./acp-runtime-config-fields";
import { NewSessionDefaultSettings } from "./new-session-default-settings";
import "./agent-engine-settings.css";

type AgentEngineSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染对话内核选择。
 *
 * 换内核换掉的是推理与决策，sai 只保留权限、沙箱、审计与会话持久化。
 * 这个落差必须摆在选择旁边——压缩与记忆会静默停摆，
 * 用户若不知情会把它当成故障。
 *
 * @param props 应用配置与更新回调
 * @returns 内核设置区域
 */
export function AgentEngineSettings({ config, onConfigChange }: AgentEngineSettingsProps) {
  const { t } = useI18n();
  const agent = (config.agent as Record<string, unknown> | undefined) ?? {};
  const engine: AgentEngineKind = typeof agent.engine === "string"
    ? agent.engine as AgentEngineKind
    : "native";
  const acp = (agent.acp as Record<string, unknown> | undefined) ?? {};
  const command = typeof acp.command === "string" ? acp.command : "";
  const isExternal = engine !== "native";
  const engineStatus = useQuery({
    queryKey: ["engine-status"],
    queryFn: api.config.engineStatus,
    enabled: isExternal,
    refetchInterval: isExternal ? 2_000 : false
  });
  const runtime = engineStatus.data?.engine === engine ? engineStatus.data.acp_runtime : undefined;

  /**
   * 合并补丁并回写内核配置。
   *
   * @param patch 待合并的配置片段
   * @returns 无返回值
   */
  const updateAgent = (patch: Record<string, unknown>) => {
    onConfigChange({ ...config, agent: { ...agent, ...patch } });
  };

  /**
   * 合并 ACP 配置补丁。
   *
   * @param patch ACP 字段补丁
   * @returns 无返回值
   */
  const updateAcp = (patch: Record<string, unknown>) => {
    updateAgent({ acp: { ...acp, ...patch } });
  };

  /**
   * 【设置】【对话内核切换】切换内核并重置相关的新会话默认值。
   *
   * @param value 新内核标识
   * @returns 无返回值
   */
  const updateEngine = (value: AgentEngineKind) => {
    onConfigChange({
      ...config,
      agent: { ...agent, engine: value },
      session: resetNewSessionEnginePreferences(config.session)
    });
  };

  const engineOptions: SelectOption<AgentEngineKind>[] = [
    {
      value: "native",
      label: t("Native", "内置内核"),
      description: t("sai's own loop with full feature set", "sai 自带循环，功能完整"),
      icon: <AgentEngineBrandIcon engine="native" size={16} />
    },
    {
      value: "claude_code",
      label: "Claude Code",
      description: t("Runs via Sai Claude Agent ACP Sidecar", "经 Sai Claude Agent ACP Sidecar 运行"),
      icon: <AgentEngineBrandIcon engine="claude_code" size={16} />
    },
    {
      value: "codex",
      label: "Codex",
      description: t("Runs via @agentclientprotocol/codex-acp", "经 @agentclientprotocol/codex-acp 运行"),
      icon: <AgentEngineBrandIcon engine="codex" size={16} />
    },
    {
      value: "custom",
      label: t("Custom ACP agent", "自定义 ACP 内核"),
      description: t("Provide your own launch command", "自行提供启动命令"),
      icon: <AgentEngineBrandIcon engine="custom" size={16} />
    }
  ];

  return (
    <div className="agent-engine-settings">
      <div className="settings-field">
        <span>{t("Conversation engine", "对话内核")}</span>
        <Select
          value={engine}
          options={engineOptions}
          onChange={updateEngine}
          ariaLabel={t("Conversation engine", "对话内核")}
        />
        <small>
          {t(
            "Which engine runs the reasoning loop. sai keeps handling permissions, sandboxing, auditing, and session history either way.",
            "由哪个内核执行推理循环。无论选哪个，权限、沙箱、审计与会话历史都仍由 sai 负责。"
          )}
        </small>
      </div>
      {isExternal && (
        <AcpCapabilityPanel
          engine={engine}
          status={engineStatus.data}
          loading={engineStatus.isLoading}
          error={engineStatus.error}
        />
      )}
      {isExternal && (
        <div className="settings-form-grid">
          <AcpRuntimeConfigFields acp={acp} runtime={runtime} onChange={updateAcp} />
          <label className="settings-field">
            <span>{t("Additional directories", "附加目录")}</span>
            <input
              type="text"
              value={Array.isArray(acp.additional_directories) ? acp.additional_directories.join(", ") : ""}
              onChange={(event) => updateAcp({ additional_directories: event.target.value.split(",").map((value) => value.trim()).filter(Boolean) })}
            />
          </label>
        </div>
      )}
      {engine === "custom" && (
        <div className="settings-field">
          <span>{t("Launch command", "启动命令")}</span>
          <input
            type="text"
            value={command}
            placeholder="npx -y @agentclientprotocol/codex-acp"
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => {
              // 首段是程序，其余作为参数；预置内核留空即用内置命令
              const parts = event.target.value.split(/\s+/).filter(Boolean);
              updateAcp({ command: parts[0] ?? "", args: parts.slice(1) });
            }}
          />
          <small>
            {t(
              "Required for the custom engine. Also lets you pin an adapter version for the preset engines.",
              "自定义内核必填。预置内核也可用它固定适配器版本。"
            )}
          </small>
        </div>
      )}
      <NewSessionDefaultSettings
        config={config}
        status={engineStatus.data}
        onConfigChange={onConfigChange}
      />
    </div>
  );
}
