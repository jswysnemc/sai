import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { AgentEngineKind, AppConfig } from "../../../api/contracts";
import { FieldGrid, SettingsField, SkListInput, SkSelect, SkTextInput, type SelectOption } from "../kit";
import { AgentEngineBrandIcon } from "../../../shared/ui/agent-engine-brand-icon/agent-engine-brand-icon";
import { useI18n } from "../../i18n/use-i18n";
import { resetNewSessionEnginePreferences } from "../../sessions/new-session-preferences";
import { AcpCapabilityPanel } from "./acp-capability-panel";
import { AcpConnectionFields } from "./acp-connection-fields";
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
      <SettingsField label={t("Conversation engine", "对话内核")} configKey="agent.engine" anchor="runtime.agent.engine" size="lg" hint={t("sai manages permissions, sandboxing, auditing, and session history for every engine.", "所有内核均由 sai 管理权限、沙箱、审计与会话历史。")}>
        <SkSelect
          value={engine}
          options={engineOptions}
          onChange={updateEngine}
          ariaLabel={t("Conversation engine", "对话内核")}
        />
      </SettingsField>
      {isExternal && (
        <AcpCapabilityPanel
          engine={engine}
          status={engineStatus.data}
          loading={engineStatus.isLoading}
          error={engineStatus.error}
        />
      )}
      {isExternal && (
        <FieldGrid>
          <AcpConnectionFields acp={acp} runtime={runtime} onChange={updateAcp} />
          <SettingsField label={t("Additional directories", "附加目录")} configKey="agent.acp.additional_directories" anchor="runtime.agent.acp.additional_directories" hint={t("One directory path per line; spaces inside a path are preserved.", "每行填写一个目录路径，保留路径中的空格。")}>
            <SkListInput
              value={Array.isArray(acp.additional_directories) ? acp.additional_directories as string[] : []}
              onChange={(value) => updateAcp({ additional_directories: value })}
            />
          </SettingsField>
        </FieldGrid>
      )}
      {engine === "custom" && (
        <FieldGrid>
        <SettingsField label={t("Launch command", "启动命令")} configKey="agent.acp.command" anchor="runtime.agent.acp.command" hint={t("Executable name or path. Enter startup arguments separately.", "填写可执行文件名称或路径，启动参数单独填写。")}>
          <SkTextInput
            value={command}
            placeholder="npx"
            spellCheck={false}
            autoComplete="off"
            onChange={(value) => updateAcp({ command: value })}
          />
        </SettingsField>
        <SettingsField label={t("Startup arguments", "启动参数")} configKey="agent.acp.args" anchor="runtime.agent.acp.args" hint={t("One argument per line; spaces within each argument are preserved.", "每行填写一个参数，保留参数内部的空格。")}>
          <SkListInput value={Array.isArray(acp.args) ? acp.args as string[] : []} onChange={(value) => updateAcp({ args: value })} />
        </SettingsField>
        </FieldGrid>
      )}
      <NewSessionDefaultSettings
        config={config}
        status={engineStatus.data}
        onConfigChange={onConfigChange}
      />
    </div>
  );
}
