import { useEffect, useRef, useState } from "react";
import { ArrowRight, Bot, Cable, CheckCircle2, Cpu, FileText, RadioTower, ShieldAlert } from "../../shared/ui/icons";
import type {
  ChannelInboundPayload,
  SuperGroupMember,
  SuperGroupMessage
} from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";

type GroupChatPanelProps = {
  messages: SuperGroupMessage[];
  members: SuperGroupMember[];
  draft: string;
  sending: boolean;
  onDraftChange: (value: string) => void;
  onSendGroupMessage: (content: string) => Promise<void>;
  onSendChannelInbound: (payload: ChannelInboundPayload) => Promise<void>;
};

/**
 * 【超级智能体】【群组对话面板】展示用户、Leader、Runner 成员间的多角色对话、结构化报告与 Leader 成果交付，
 * 并支持在群组直聊与渠道入站（微信/飞书/QQ/OneBot）两种输入模式间切换。
 *
 * @param props 群聊消息、成员列表与发送回调
 * @returns 群组聊天区域
 */
export function GroupChatPanel({
  messages,
  members,
  draft,
  sending,
  onDraftChange,
  onSendGroupMessage,
  onSendChannelInbound
}: GroupChatPanelProps) {
  const { t } = useI18n();
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const [inputMode, setInputMode] = useState<"group" | "channel">("group");
  const [channel, setChannel] = useState("weixin");
  const [chatId, setChatId] = useState("wx_user_1");
  const [actor, setActor] = useState("ChannelUser");
  const [simulateOutbound, setSimulateOutbound] = useState(true);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  const handleSubmit = async () => {
    const text = draft.trim();
    if (!text) return;
    onDraftChange("");
    if (inputMode === "group") {
      await onSendGroupMessage(text);
    } else {
      await onSendChannelInbound({
        channel,
        external_chat_id: chatId.trim() || "default",
        actor: actor.trim() || `${channel}_user`,
        content: text,
        simulate_outbound: simulateOutbound,
        auto_dispatch: true
      });
    }
  };

  return (
    <div className="sa-chat-wrap">
      <div className="sa-chat-timeline" role="log" aria-label={t("Group chat timeline", "群组对话时间线")}>
        {messages.length === 0 && (
          <div className="sa-empty">
            {t(
              "No messages yet. Ask the Leader to orchestrate tasks or use @runner to coordinate directly.",
              "暂无群组消息。直接向 Leader 下达需求，或使用 @Runner名 指定成员协同。"
            )}
          </div>
        )}
        {messages.map((msg) => (
          <MessageCard key={msg.id} message={msg} />
        ))}
        <div ref={bottomRef} />
      </div>

      <div className="sa-composer">
        <div className="sa-composer-controls">
          <Button
            size="small"
            variant={inputMode === "group" ? "primary" : "secondary"}
            onClick={() => setInputMode("group")}
          >
            {t("Group chat", "群组对话")}
          </Button>
          <Button
            size="small"
            variant={inputMode === "channel" ? "primary" : "secondary"}
            onClick={() => setInputMode("channel")}
          >
            <Cable size={14} />
            {t("Channel -> Leader", "渠道对接 Leader")}
          </Button>

          {inputMode === "group" && members.map((m) => (
            <button
              key={m.id}
              type="button"
              className="sa-badge"
              style={{ cursor: "pointer" }}
              onClick={() => onDraftChange(draft.startsWith(`@${m.name} `) ? draft : `@${m.name} ${draft}`)}
            >
              @{m.name}
            </button>
          ))}

          {inputMode === "channel" && (
            <>
              <select className="sa-select" value={channel} onChange={(e) => setChannel(e.target.value)} aria-label={t("Channel", "渠道")}>
                <option value="weixin">Weixin</option>
                <option value="feishu">Feishu</option>
                <option value="qq">QQ</option>
                <option value="onebot">OneBot</option>
              </select>
              <input
                className="sa-input"
                value={chatId}
                onChange={(e) => setChatId(e.target.value)}
                placeholder={t("External chat ID", "外部会话 ID")}
                style={{ width: "8.5rem" }}
              />
              <input
                className="sa-input"
                value={actor}
                onChange={(e) => setActor(e.target.value)}
                placeholder={t("Sender", "发送人")}
                style={{ width: "7.5rem" }}
              />
              <label style={{ display: "inline-flex", alignItems: "center", gap: "0.25rem", fontSize: "0.75rem" }}>
                <input
                  type="checkbox"
                  checked={simulateOutbound}
                  onChange={(e) => setSimulateOutbound(e.target.checked)}
                />
                {t("Simulate outbound", "模拟渠道回推")}
              </label>
            </>
          )}
        </div>

        <div className="sa-composer-row">
          <textarea
            className="sa-textarea"
            value={draft}
            onChange={(e) => onDraftChange(e.target.value)}
            onKeyDown={(ev) => {
              if (ev.key === "Enter" && !ev.shiftKey) {
                ev.preventDefault();
                void handleSubmit();
              }
            }}
            placeholder={
              inputMode === "group"
                ? t("Message the Leader or @runner... (Enter to send, Shift+Enter for newline)", "向 Leader 派发目标或 @Runner 沟通...（Enter 发送，Shift+Enter 换行）")
                : t("Send a channel message to the Leader; completed Runner work will be synthesized and delivered back...", "模拟或发起渠道消息给 Leader，Runner 完成后将由 Leader 主动归纳并递交成果...")
            }
          />
          <Button variant="primary" disabled={sending || !draft.trim()} onClick={() => void handleSubmit()}>
            <ArrowRight size={16} />
            {sending ? t("Sending", "发送中") : t("Send", "发送")}
          </Button>
        </div>
      </div>
    </div>
  );
}

/** 渲染单条多角色群组消息卡片。 */
function MessageCard({ message }: { message: SuperGroupMessage }) {
  const { t } = useI18n();
  const cardClass = [
    "sa-msg-card",
    message.sender_kind === "user" && message.kind !== "channel_in" ? "is-user" : "",
    message.kind === "delivery" ? "is-delivery" : "",
    message.kind === "report" ? "is-report" : ""
  ]
    .filter(Boolean)
    .join(" ");

  const roleIcon = (() => {
    if (message.kind === "delivery") return <CheckCircle2 size={14} />;
    if (message.kind === "report") return <FileText size={14} />;
    if (message.kind === "channel_in") return <RadioTower size={14} />;
    if (message.sender_kind === "watchdog") return <ShieldAlert size={14} />;
    if (message.sender_kind === "leader") return <Bot size={14} />;
    if (message.sender_kind === "worker") return <Cpu size={14} />;
    return null;
  })();

  const kindLabel: Record<string, string> = {
    chat: message.sender_kind === "leader" ? "Leader" : message.sender_kind === "worker" ? "Runner" : t("Chat", "对话"),
    dispatch: t("Dispatch", "派工"),
    report: t("Runner Report", "Runner 报告"),
    delivery: t("Leader Delivery", "Leader 成果交付"),
    progress: t("Progress", "进度"),
    alert: t("Watchdog Alert", "守护告警"),
    channel_in: t("Channel Inbound", "渠道入站"),
    channel_out: t("Channel Outbound", "渠道回推")
  };

  return (
    <article className={cardClass}>
      <header className="sa-msg-head">
        {roleIcon}
        <span className="sa-msg-sender">{message.sender_name}</span>
        {message.recipient_name && <span>→ @{message.recipient_name}</span>}
        <span className={message.kind === "delivery" ? "sa-badge accent" : message.sender_kind === "watchdog" ? "sa-badge danger" : "sa-badge"}>
          {kindLabel[message.kind] ?? message.kind}
        </span>
        {message.channel_source && !["group", "web", "tui"].includes(message.channel_source) && (
          <span className="sa-badge">via {message.channel_source}</span>
        )}
        <span>{new Date(message.created_at).toLocaleTimeString()}</span>
      </header>
      <p className="sa-msg-body">{message.content}</p>
    </article>
  );
}
