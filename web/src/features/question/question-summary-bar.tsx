import { Check, ChevronDown, MessageSquareText, X } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import type { CardStatus } from "./question-card-state";

type QuestionSummaryBarProps = {
  status: CardStatus;
  label: string;
  /** 折叠态展示的一行摘要：已回答时为选择结果，否则为问题标题 */
  summary: string;
  expanded: boolean;
  onToggle: () => void;
};

/**
 * 渲染已处理提问的单行摘要条，点击展开或收起历史选择。
 *
 * @param props 状态、状态文案、摘要与展开控制
 * @returns 单行摘要按钮
 */
export function QuestionSummaryBar({ status, label, summary, expanded, onToggle }: QuestionSummaryBarProps) {
  const StatusIcon = status === "answered" ? Check : status === "pending" ? MessageSquareText : X;
  return (
    <Button variant="ghost" className="question-summary-bar" onClick={onToggle} aria-expanded={expanded}>
      <span className="question-summary-line icon-label">
        <StatusIcon size={14} className="question-status-icon" aria-hidden />
        <strong>{label}</strong>
        <span className="question-summary-text" title={summary}>{summary}</span>
      </span>
      <ChevronDown size={14} className={`question-summary-chevron${expanded ? " is-open" : ""}`} aria-hidden />
    </Button>
  );
}
