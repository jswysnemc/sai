import { Check, ChevronDown, HelpCircle, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import type { PendingQuestion, QuestionAnswers, QuestionPrompt, QuestionResponse } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { TextArea } from "../../shared/ui/form/text-area";
import "./question-request-card.css";

type QuestionRequestCardProps = {
  pending: PendingQuestion;
  response?: QuestionResponse;
  active?: boolean;
};

type CardStatus = "pending" | "answered" | "cancelled" | "unavailable";

/**
 * 在助手消息流内渲染可交互结构化提问卡片。
 */
export function QuestionRequestCard({ pending, response, active = true }: QuestionRequestCardProps) {
  const questions = pending.request.questions;
  const [status, setStatus] = useState<CardStatus>(() => responseStatus(response));
  const [expanded, setExpanded] = useState(true);
  const [tab, setTab] = useState(0);
  const [answers, setAnswers] = useState<QuestionAnswers>(() => questions.map(() => []));
  const [customDrafts, setCustomDrafts] = useState<string[]>(() => questions.map(() => ""));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");
  const [resolvedSummary, setResolvedSummary] = useState<string[]>(() => summaryFromResponse(response));

  useEffect(() => {
    setStatus(responseStatus(response));
    setExpanded(true);
    setSubmitting(false);
    setError("");
    setResolvedSummary(summaryFromResponse(response));
  }, [pending.id, response]);

  const current = questions[tab] ?? questions[0];
  const allAnswered = useMemo(() => answers.every((item) => item.length > 0), [answers]);

  const toggleOption = (questionIndex: number, label: string, multiple: boolean) => {
    setAnswers((prev) => {
      const next = prev.map((item) => [...item]);
      const selected = next[questionIndex] ?? [];
      if (multiple) {
        next[questionIndex] = selected.includes(label)
          ? selected.filter((item) => item !== label)
          : [...selected, label];
      } else {
        next[questionIndex] = [label];
      }
      return next;
    });
  };

  const saveCustom = (questionIndex: number, multiple: boolean) => {
    const value = (customDrafts[questionIndex] ?? "").trim();
    if (!value) return;
    setAnswers((prev) => {
      const next = prev.map((item) => [...item]);
      if (multiple) {
        const selected = next[questionIndex] ?? [];
        next[questionIndex] = selected.includes(value) ? selected : [...selected, value];
      } else {
        next[questionIndex] = [value];
      }
      return next;
    });
  };

  const submit = async () => {
    if (!allAnswered) {
      setError("请先回答所有问题");
      return;
    }
    setSubmitting(true);
    setError("");
    try {
      await api.questions.answer(pending.id, answers);
      setStatus("answered");
      setResolvedSummary(answers.map((item) => item.join("、")));
      setExpanded(false);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "提交回答失败");
    } finally {
      setSubmitting(false);
    }
  };

  const cancel = async () => {
    setSubmitting(true);
    setError("");
    try {
      await api.questions.cancel(pending.id);
      setStatus("cancelled");
      setExpanded(false);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "取消提问失败");
    } finally {
      setSubmitting(false);
    }
  };

  const resolved = status !== "pending";
  const interactive = !resolved && active;

  return (
    <section className={`question-request-card is-${status}`}>
      <Button className="question-request-head" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <span className="question-request-icon" aria-hidden>
          {status === "answered" ? <Check size={14} /> : status === "cancelled" || status === "unavailable" ? <X size={14} /> : <HelpCircle size={14} />}
        </span>
        <span className="question-request-copy">
          <strong>{statusLabel(status, active)}</strong>
          <span>{questions.length} 个问题 · {questions.map((item) => item.header).join(" / ")}</span>
        </span>
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </Button>
      {expanded && (
        <div className="question-request-body">
          {questions.length > 1 && (
            <div className="question-tabs">
              {questions.map((question, index) => (
                <button
                  key={`${question.header}-${index}`}
                  type="button"
                  className={`question-tab ${tab === index ? "is-active" : ""} ${answers[index]?.length ? "is-answered" : ""}`}
                  onClick={() => setTab(index)}
                >
                  {question.header}
                </button>
              ))}
            </div>
          )}
          {current && (
            <QuestionPanel
              question={current}
              selected={answers[tab] ?? []}
              customDraft={customDrafts[tab] ?? ""}
              interactive={interactive}
              onToggle={(label) => toggleOption(tab, label, Boolean(current.multiple))}
              onCustomDraft={(value) => setCustomDrafts((prev) => prev.map((item, index) => (index === tab ? value : item)))}
              onSaveCustom={() => saveCustom(tab, Boolean(current.multiple))}
            />
          )}
          {resolved && resolvedSummary.length > 0 && (
            <div className="question-resolved-summary">
              {questions.map((question, index) => (
                <div key={`${question.header}-summary-${index}`}>
                  <span>{question.header}</span>
                  {resolvedSummary[index] || "未回答"}
                </div>
              ))}
            </div>
          )}
          {interactive && (
            <div className="question-request-actions">
              {error && <div className="question-request-error">{error}</div>}
              <div className="question-request-buttons">
                <Button className="question-action" disabled={submitting} onClick={() => void cancel()}>
                  {submitting ? "处理中" : "取消"}
                </Button>
                <Button variant="primary" className="question-action" disabled={submitting || !allAnswered} onClick={() => void submit()}>
                  {submitting ? "提交中" : "提交回答"}
                </Button>
              </div>
            </div>
          )}
          {!resolved && !active && <div className="question-request-ended">提问已随本轮运行结束</div>}
        </div>
      )}
    </section>
  );
}

function QuestionPanel({
  question,
  selected,
  customDraft,
  interactive,
  onToggle,
  onCustomDraft,
  onSaveCustom
}: {
  question: QuestionPrompt;
  selected: string[];
  customDraft: string;
  interactive: boolean;
  onToggle: (label: string) => void;
  onCustomDraft: (value: string) => void;
  onSaveCustom: () => void;
}) {
  const multiple = Boolean(question.multiple);
  const allowCustom = question.custom !== false;
  return (
    <div className="question-panel">
      <div className="question-text">{question.question}</div>
      <div className="question-options">
        {question.options.map((option) => {
          const active = selected.includes(option.label);
          return (
            <button
              key={option.label}
              type="button"
              className={`question-option ${active ? "is-selected" : ""}`}
              disabled={!interactive}
              onClick={() => onToggle(option.label)}
            >
              <strong>{multiple ? (active ? "[✓] " : "[ ] ") : ""}{option.label}</strong>
              {option.description && <span>{option.description}</span>}
            </button>
          );
        })}
      </div>
      {allowCustom && interactive && (
        <label className="question-custom">
          <span>自定义答案</span>
          <TextArea value={customDraft} onChange={(event) => onCustomDraft(event.target.value)} placeholder="输入其他答案" />
          <Button className="question-custom-save" disabled={!customDraft.trim()} onClick={onSaveCustom}>
            使用自定义答案
          </Button>
        </label>
      )}
    </div>
  );
}

function statusLabel(status: CardStatus, active: boolean): string {
  if (status === "pending" && !active) return "提问已结束";
  return {
    pending: "需要你的回答",
    answered: "已回答",
    cancelled: "已取消",
    unavailable: "无法提问"
  }[status];
}

function responseStatus(response?: QuestionResponse): CardStatus {
  if (!response) return "pending";
  if (response.status === "answered") return "answered";
  if (response.status === "cancelled") return "cancelled";
  return "unavailable";
}

function summaryFromResponse(response?: QuestionResponse): string[] {
  if (!response || response.status !== "answered") return [];
  return response.data.map((item) => item.join("、"));
}
