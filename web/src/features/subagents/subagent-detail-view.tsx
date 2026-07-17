import { ArrowLeft, Ban } from "lucide-react";
import { useEffect, useRef } from "react";
import type { Subagent } from "../../api/contracts";
import { MessageParts } from "../chat/message/message-parts";
import { SubagentProgress } from "./subagent-progress";
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
 * 渲染子智能体详情:元信息、实时进度、流式时间线与 Markdown 结果输出。
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
  const scrollRef = useRef<HTMLDivElement>(null);
  const timeline = stream.timeline;
  const parts = subagentMessageParts(timeline, running, stream.timestamp, locale);
  const body = current.result || current.error || "";
  if (body && !timeline.some((entry) => entry.kind === "text" && entry.text === body)) {
    parts.push({ id: "subagent-result", type: "text", source: body });
  }

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
        {running && (
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
        <SubagentProgress subagent={current} />
        {parts.length > 0 ? <MessageParts parts={parts} live={running} /> : (
          <p className="subagent-detail-pending">{running ? t("The subagent is running. Its progress will appear here in real time.", "子智能体正在运行，执行过程会在此实时显示。") : t("No output.", "没有输出。")}</p>
        )}
      </div>
    </section>
  );
}
