import {
  Bot,
  Check,
  CircleDashed,
  Cpu,
  Loader2,
  TerminalSquare,
  X
} from "lucide-react";
import type { ReactNode } from "react";
import type { AgentEngineKind, EngineStatusResponse } from "../../../api/contracts";
import type { Translate } from "../../i18n/i18n-context";
import { useI18n } from "../../i18n/use-i18n";
import {
  capabilitiesForEngine,
  groupAcpCapabilities,
  parseAcpCommands,
  resolveAcpConnectionState,
  type AcpCapabilityItem,
  type AcpConnectionState
} from "./acp-capability-data";
import "./acp-capability-panel.css";

type AcpCapabilityPanelProps = {
  engine: AgentEngineKind;
  status: EngineStatusResponse | undefined;
  loading: boolean;
  error: unknown;
};

/**
 * 渲染外部内核最近一次 ACP 握手与 Sai 集成能力。
 *
 * @param props 当前内核、查询状态和服务端运行快照
 * @returns ACP 连接状态、能力分组和斜杠命令面板
 */
export function AcpCapabilityPanel({ engine, status, loading, error }: AcpCapabilityPanelProps) {
  const { locale, t } = useI18n();
  const state = resolveAcpConnectionState(engine, status, loading, error);
  const runtime = status?.engine === engine ? status.acp_runtime : null;
  const capabilities = capabilitiesForEngine(engine, status);
  const groups = groupAcpCapabilities(engine, capabilities, runtime?.native_equivalents);
  const commands = parseAcpCommands(runtime?.available_commands);
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : "";

  return (
    <section className={`acp-capability-panel state-${state}`} aria-live="polite">
      <header className="acp-capability-head">
        <div className="acp-capability-heading">
          <span className="acp-capability-heading-icon" aria-hidden><Cpu size={15} /></span>
          <span>
            <strong>{t("ACP runtime capabilities", "ACP 运行能力")}</strong>
            <small>{t("Reported by the latest initialize handshake", "来自最近一次 initialize 握手")}</small>
          </span>
        </div>
        <AcpStatusBadge state={state} t={t} />
      </header>

      {state === "loading" && (
        <AcpPanelMessage
          icon={<Loader2 className="acp-capability-spinner" />}
          title={t("Loading runtime status", "正在加载运行状态")}
          detail={t("Checking the selected ACP engine.", "正在查询所选 ACP 内核。")}
        />
      )}
      {state === "disconnected" && (
        <AcpPanelMessage
          icon={<CircleDashed />}
          title={runtime
            ? t("Process disconnected", "当前未连接")
            : t("No handshake yet", "尚未完成握手")}
          detail={runtime
            ? t(
                "The process is disconnected. The latest handshake snapshot remains available below.",
                "当前进程已断开，下方保留最近一次握手快照。"
              )
            : t(
                "Save this engine and start a conversation. Runtime capabilities will appear after the agent connects.",
                "保存当前内核并开始一次对话。内核连接后，此处会显示运行能力。"
              )}
        />
      )}
      {state === "error" && (
        <AcpPanelMessage
          icon={<X />}
          title={t("Runtime status query failed", "运行状态查询失败")}
          detail={errorMessage || t("The server did not return an ACP runtime status.", "服务端未返回 ACP 运行状态。")}
          danger
        />
      )}

      {(state === "connected" || state === "partial" || state === "disconnected") && runtime && (
        <div className="acp-capability-content">
          <div className="acp-runtime-summary">
            <div className="acp-runtime-agent">
              <Bot size={15} aria-hidden />
              <span>
                <strong>{runtime.agent_name || status?.label || t("ACP agent", "ACP 内核")}</strong>
                <small>
                  {runtime.agent_version
                    ? t(`Version ${runtime.agent_version}`, `版本 ${runtime.agent_version}`)
                    : t("Version not reported", "未公布版本")}
                </small>
              </span>
            </div>
            <div className="acp-runtime-commands">
              <span className="acp-runtime-label"><TerminalSquare size={13} aria-hidden />{t("Slash commands", "斜杠命令")}</span>
              {commands.length > 0 ? (
                <div className="acp-command-list">
                  {commands.map((command) => (
                    <span className="acp-command" key={command.name} title={command.description || command.name}>
                      {command.name}
                    </span>
                  ))}
                </div>
              ) : (
                <small>{t("The agent has not published commands.", "内核尚未公布命令。")}</small>
              )}
            </div>
          </div>

          <div className="acp-capability-groups">
            <AcpCapabilityGroup
              title={t("Standard ACP capabilities", "标准 ACP 能力")}
              detail={t("Negotiated directly through the protocol", "由协议直接协商")}
              items={groups.standard}
              locale={locale}
              supported
            />
            <AcpCapabilityGroup
              title={t("Sai integration capabilities", "Sai 集成能力")}
              detail={t("Connected by Sai and the adapter", "由 Sai 与适配器共同接通")}
              items={groups.sai}
              locale={locale}
              supported
            />
            <AcpCapabilityGroup
              title={t("Codex native equivalents", "Codex 原生等价能力")}
              detail={t("Provided by native Codex events", "由 Codex 原生事件提供")}
              items={groups.codexNative}
              locale={locale}
              supported
            />
            <AcpCapabilityGroup
              title={t("Unsupported capabilities", "未支持能力")}
              detail={t("Not reported by this handshake", "本次握手未公布")}
              items={groups.unsupported}
              locale={locale}
              supported={false}
            />
          </div>
        </div>
      )}
    </section>
  );
}

/**
 * 渲染 ACP 查询状态徽章。
 *
 * @param props 状态与翻译方法
 * @returns 状态徽章
 */
function AcpStatusBadge({ state, t }: { state: AcpConnectionState; t: Translate }) {
  const labels: Record<AcpConnectionState, string> = {
    loading: t("Loading", "加载中"),
    disconnected: t("Not connected", "尚未连接"),
    connected: t("Connected", "已连接"),
    partial: t("Partial capabilities", "部分能力"),
    error: t("Query failed", "查询失败")
  };
  return <span className={`acp-status-badge is-${state}`}>{labels[state]}</span>;
}

/**
 * 渲染加载、空状态或错误说明。
 *
 * @param props 图标、标题、详情与危险状态
 * @returns 紧凑状态说明
 */
function AcpPanelMessage({
  icon,
  title,
  detail,
  danger = false
}: {
  icon: ReactNode;
  title: string;
  detail: string;
  danger?: boolean;
}) {
  return (
    <div className={`acp-panel-message${danger ? " is-danger" : ""}`} role={danger ? "alert" : undefined}>
      <span aria-hidden>{icon}</span>
      <div><strong>{title}</strong><small>{detail}</small></div>
    </div>
  );
}

/**
 * 渲染一组同来源能力。
 *
 * @param props 分组标题、说明、能力条目、语言和支持状态
 * @returns 能力标签分组；空分组不占据页面空间
 */
function AcpCapabilityGroup({
  title,
  detail,
  items,
  locale,
  supported
}: {
  title: string;
  detail: string;
  items: AcpCapabilityItem[];
  locale: "en-US" | "zh-CN";
  supported: boolean;
}) {
  if (items.length === 0) return null;
  return (
    <section className={`acp-capability-group${supported ? "" : " is-unsupported"}`}>
      <header><strong>{title}</strong><small>{detail}</small></header>
      <div className="acp-capability-list">
        {items.map((item) => (
          <span
            className="acp-capability-item"
            key={item.id}
            title={locale === "zh-CN" ? item.description.zh : item.description.en}
          >
            {supported ? <Check size={12} aria-hidden /> : <X size={12} aria-hidden />}
            {locale === "zh-CN" ? item.label.zh : item.label.en}
          </span>
        ))}
      </div>
    </section>
  );
}
