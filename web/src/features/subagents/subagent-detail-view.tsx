import { ArrowLeft, ArrowRight, Ban } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import type { Subagent } from "../../api/contracts";
import { ComposerSurface } from "../chat/composer/composer-surface";
import { MessageParts } from "../chat/message/message-parts";
import { SubagentStats } from "./subagent-stats";
import { SubagentStatusBadge } from "./subagent-status-badge";
import { subagentDuration, subagentTypeLabel } from "./subagent-labels";
import { subagentMessageParts } from "./subagent-message-parts";
import { useSubagentStream } from "./use-subagent-stream";
import { useI18n } from "../i18n/use-i18n";

type SubagentDetailViewProps = {
  subagent: Subagent;
  onBack: () => void;
  onCancel: (id: string) => void;
};

/**
 * 渲染子智能体详情:元信息、状态、流式时间线与 Markdown 结果输出。
 *
 * 运行中通过 SSE 接收详情快照，时间线随执行增量出现；新内容到达时若视口
 * 停留在底部附近则自动跟随滚动。
 *
 * @param props 子智能体列表快照与返回、取消回调
 * @returns 子智能体详情视图
 */
export function SubagentDetailView({ subagent, onBack, onCancel }: SubagentDetailViewProps) {
  const { locale, t } = useI18n();
  const stream = useSubagentStream(subagent);
  const current = stream.snapshot;
  const running = current.status === "running";
  // 存活（运行中或待命中）的子智能体可接收用户留言
  const alive = running || current.status === "idle";
  const [draft, setDraft] = useState("");
  const [sendError, setSendError] = useState("");
  const [sending, setSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const timeline = stream.timeline;
  const inputDisabled = sending || !alive;
  const submitDisabled = inputDisabled || !draft.trim();
  const messagePlaceholder = alive
    ? t("Leave a message; it is injected at the next step boundary", "给子智能体留言，将在下一个步间间隙注入")
    : t("The subagent has finished and no longer accepts messages", "子智能体已结束，不再接收留言");
  const parts = subagentMessageParts(timeline, running, stream.timestamp, locale);
  const body = current.result || current.error || "";
  if (body && !timeline.some((entry) => entry.kind === "text" && entry.text === body)) {
    parts.push({ id: "subagent-result", type: "text", source: body });
  }

  const sendMessage = async () => {
    const message = draft.trim();
    if (!message || sending) return;
    setSending(true);
    setSendError("");
    try {
      await api.subagents.message(current.id, message);
      setDraft("");
    } catch (error) {
      setSendError(error instanceof Error ? error.message : String(error));
    } finally {
      setSending(false);
    }
  };

  useEffect(() => {
    // 1. 视口停在底部附近时，新时间线内容到达后自动跟随到底
    const node = scrollRef.current;
    if (!node || !running) return;
    const nearBottom = node.scrollHeight - node.scrollTop - node.clientHeight < 120;
    if (nearBottom) node.scrollTop = node.scrollHeight;
  }, [running, timeline.length, body]);

  return (
    <section className="subagent-detail-view">
      <header className="subagent-detail-head">
        <button type="button" className="subagent-detail-back" onClick={onBack}><ArrowLeft size={14} />{t("Overview", "概览")}</button>
        <SubagentStatusBadge status={current.status} />
        {alive && (
          <button type="button" className="subagent-detail-cancel" onClick={() => onCancel(current.id)}><Ban size={13} />{t("Cancel", "取消")}</button>
        )}
      </header>
      <div className="subagent-detail-scroll" ref={scrollRef}>
        <h2 className="subagent-detail-title">{current.description}</h2>
        <dl className="subagent-detail-meta">
          <div><dt>{t("Type", "类型")}</dt><dd>{subagentTypeLabel(current.subagent_type, locale)}</dd></div>
          <div><dt>{t("Duration", "用时")}</dt><dd>{subagentDuration(current.started_at, current.updated_at)}</dd></div>
          {current.last_tool && <div><dt>{t("Latest tool", "最近工具")}</dt><dd>{current.last_tool}</dd></div>}
        </dl>
        <SubagentStats subagent={current} />
        {parts.length > 0 ? <MessageParts parts={parts} live={running} /> : (
          <p className="subagent-detail-pending">{running ? t("The subagent is running.", "子智能体正在运行。") : t("No output.", "没有输出。")}</p>
        )}
      </div>
      {/* 输入区始终可见以便发现留言能力；仅存活（运行中/待命）时可发送 */}
      <footer className="subagent-detail-composer-shell">
        <ComposerSurface
          variant="compact"
          className="composer subagent-detail-composer"
          value={draft}
          historyEntries={[]}
          disabled={inputDisabled}
          submitDisabled={submitDisabled}
          respondToGlobalFocus={false}
          placeholder={messagePlaceholder}
          onChange={setDraft}
          onSubmit={() => void sendMessage()}
        >
          {sendError && <p className="subagent-detail-send-error" role="alert">{sendError}</p>}
          <div className="composer-footer subagent-detail-composer-actions">
            <div className="composer-actions">
              <button
                type="submit"
                className="composer-send"
                disabled={submitDisabled}
                aria-label={t("Send message to subagent", "向子智能体发送留言")}
                title={t("Send message to subagent", "向子智能体发送留言")}
              >
                <ArrowRight size={18} />
              </button>
            </div>
          </div>
        </ComposerSurface>
      </footer>
    </section>
  );
}
