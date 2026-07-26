import { AlertTriangle } from "lucide-react";
import type { AppConfig } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import "./agent-engine-settings.css";

type AgentEngineSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/** 外部内核会让这些 sai 功能停用，与服务端 unavailable_features 保持一致 */
const DISABLED_BY_EXTERNAL_ENGINE: [string, string][] = [
  ["context compaction", "上下文压缩"],
  ["memory injection", "记忆注入"],
  ["goal continuation", "目标续轮"],
  ["subagents", "子智能体"],
  ["token usage stats", "token 用量统计"]
];

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
  const engine = typeof agent.engine === "string" ? agent.engine : "native";
  const acp = (agent.acp as Record<string, unknown> | undefined) ?? {};
  const command = typeof acp.command === "string" ? acp.command : "";
  const isExternal = engine !== "native";

  /**
   * 合并补丁并回写内核配置。
   *
   * @param patch 待合并的配置片段
   * @returns 无返回值
   */
  const updateAgent = (patch: Record<string, unknown>) => {
    onConfigChange({ ...config, agent: { ...agent, ...patch } });
  };

  const engineOptions = [
    {
      value: "native",
      label: t("Native", "内置内核"),
      description: t("sai's own loop with full feature set", "sai 自带循环，功能完整")
    },
    {
      value: "claude_code",
      label: "Claude Code",
      description: t("Runs via @zed-industries/claude-code-acp", "经 @zed-industries/claude-code-acp 运行")
    },
    {
      value: "codex",
      label: "Codex",
      description: t("Runs via @agentclientprotocol/codex-acp", "经 @agentclientprotocol/codex-acp 运行")
    },
    {
      value: "custom",
      label: t("Custom ACP agent", "自定义 ACP 内核"),
      description: t("Provide your own launch command", "自行提供启动命令")
    }
  ];

  return (
    <div className="agent-engine-settings">
      <div className="settings-field">
        <span>{t("Conversation engine", "对话内核")}</span>
        <Select
          value={engine}
          options={engineOptions}
          onChange={(value) => updateAgent({ engine: value })}
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
        <div className="agent-engine-warning" role="note">
          <span className="agent-engine-warning-head">
            <AlertTriangle size={14} aria-hidden />
            {t("Disabled while this engine is active", "使用该内核期间以下功能停用")}
          </span>
          <ul>
            {DISABLED_BY_EXTERNAL_ENGINE.map(([en, zh]) => (
              <li key={en}>{t(en, zh)}</li>
            ))}
          </ul>
          <small>
            {t(
              "These rely on sai assembling the context itself; an external engine maintains its own conversation history.",
              "这些能力依赖 sai 自己组装上下文，而外部内核维护自己的对话历史。"
            )}
          </small>
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
              updateAgent({
                acp: { ...acp, command: parts[0] ?? "", args: parts.slice(1) }
              });
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
    </div>
  );
}
