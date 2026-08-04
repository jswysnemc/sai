import { Check, ChevronDown, Info, MessageSquareText, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { LocalizedError, toDisplayError } from "../../api/api-error";
import type { PendingQuestion, QuestionAnswers, QuestionPrompt, QuestionResponse } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import "./question-request-card.css";
import { useI18n } from "../i18n/use-i18n";
import { QuestionOptionPanel } from "./question-option-panel";

type QuestionRequestCardProps = {
  pending: PendingQuestion;
  response?: QuestionResponse;
  active?: boolean;
};

type CardStatus = "pending" | "answered" | "cancelled" | "unavailable";

/**
 * 在助手消息流内渲染可交互结构化提问卡片。
 *
 * @param props 待回答问题、可选响应结果和当前轮次状态
 * @returns 可展开的结构化提问卡片
 */
export function QuestionRequestCard({ pending, response, active = true }: QuestionRequestCardProps) {
  const { t } = useI18n();
  const questions = pending.request.questions;
  const [status, setStatus] = useState<CardStatus>(() => responseStatus(response));
  const [expanded, setExpanded] = useState(true);
  const [tab, setTab] = useState(0);
  const [answers, setAnswers] = useState<QuestionAnswers>(() => initialAnswers(questions));
  const [customDrafts, setCustomDrafts] = useState<string[]>(() => initialCustomDrafts(questions));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [resolvedSummary, setResolvedSummary] = useState<string[]>(() => summaryFromResponse(response, t));

  useEffect(() => {
    setStatus(responseStatus(response));
    setExpanded(true);
    setSubmitting(false);
    setError(null);
    setAnswers(initialAnswers(questions));
    setCustomDrafts(initialCustomDrafts(questions));
    setResolvedSummary(summaryFromResponse(response, t));
  }, [pending.id, questions, response, t]);

  const current = questions[tab] ?? questions[0];
  const allAnswered = useMemo(
    () => answers.every((item, index) => questions[index]?.required === false || item.length > 0),
    [answers, questions]
  );
  const answeredCount = useMemo(
    () => answers.filter((item, index) => (questions[index]?.required === false ? true : item.length > 0)).length,
    [answers, questions]
  );

  /**
   * 更新指定问题的预设选项答案。
   *
   * @param questionIndex 问题索引
   * @param label 选项提交值
   * @param multiple 是否允许多选
   * @returns 无返回值
   */
  const toggleOption = (questionIndex: number, label: string, multiple: boolean): void => {
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

  /**
   * 将自定义输入保存为指定问题的答案。
   *
   * @param questionIndex 问题索引
   * @param multiple 是否允许多选
   * @returns 无返回值
   */
  const saveCustom = (questionIndex: number, multiple: boolean): void => {
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

  /**
   * 校验并提交全部问题答案。
   *
   * @returns 提交完成后的 Promise
   */
  const submit = async (): Promise<void> => {
    if (!allAnswered) {
      setError(new LocalizedError("Answer every question first", "请先回答所有问题"));
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await api.questions.answer(pending.id, answers);
      setStatus("answered");
      setResolvedSummary(answers.map((item) => item.join(t(", ", "、"))));
      setExpanded(false);
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to submit answers", "提交回答失败"));
    } finally {
      setSubmitting(false);
    }
  };

  /**
   * 取消当前结构化提问请求。
   *
   * @returns 取消完成后的 Promise
   */
  const cancel = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    try {
      await api.questions.cancel(pending.id);
      setStatus("cancelled");
      setExpanded(false);
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to cancel questions", "取消提问失败"));
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
          {status === "answered" ? <Check size={14} /> : status === "cancelled" || status === "unavailable" ? <X size={14} /> : <MessageSquareText size={14} />}
        </span>
        <span className="question-request-copy">
          <strong>{statusLabel(status, active, t)}</strong>
          <span>{t(`${questions.length} questions`, `${questions.length} 个问题`)} · {questions.map((item) => item.header).join(" / ")}</span>
        </span>
        {status === "pending" && (
          <span className="question-request-progress" aria-label={t(`${answeredCount} of ${questions.length} answered`, `已回答 ${answeredCount}/${questions.length}`)}>
            {answeredCount}/{questions.length}
          </span>
        )}
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </Button>
      {expanded && (
        <div className="question-request-body">
          {questions.length > 1 && (
            <div className="question-tabs">
              {questions.map((question, index) => (
                <Button
                  key={`${question.header}-${index}`}
                  className={`question-tab ${tab === index ? "is-active" : ""} ${answers[index]?.length ? "is-answered" : ""}`}
                  onClick={() => setTab(index)}
                >
                  {question.header}
                  {Boolean(answers[index]?.length) && <Check size={11} className="question-tab-check" aria-hidden />}
                </Button>
              ))}
            </div>
          )}
          {current && (
            <QuestionOptionPanel
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
                  {resolvedSummary[index] || (question.required === false
                    ? t("Skipped", "已跳过")
                    : t("Unanswered", "未回答"))}
                </div>
              ))}
            </div>
          )}
          {interactive && (
            <div className="question-request-actions">
              {error && <div className="question-request-error">{error.message}</div>}
              <div className="question-request-footer">
                <div className="question-request-hint">
                  <Info size={14} aria-hidden />
                  <span>{t("Use Tab or arrow keys to select, then confirm", "使用 Tab 或方向键选择，然后确认")}</span>
                </div>
                <div className="question-request-buttons">
                  <Button className="question-action question-cancel" disabled={submitting} onClick={() => void cancel()}>
                    {submitting ? t("Processing", "处理中") : t("Cancel", "取消")}
                  </Button>
                  <Button variant="primary" className="question-action" disabled={submitting || !allAnswered} onClick={() => void submit()}>
                    {submitting ? t("Submitting", "提交中") : t("Confirm", "确认")}
                  </Button>
                </div>
              </div>
            </div>
          )}
          {!resolved && !active && <div className="question-request-ended">{t("The questions ended with this run", "提问已随本轮运行结束")}</div>}
        </div>
      )}
    </section>
  );
}

/**
 * 从问题默认值构造初始回答。
 *
 * @param questions 当前问题集合
 * @returns 每个问题对应的初始回答
 */
function initialAnswers(questions: QuestionPrompt[]): QuestionAnswers {
  return questions.map((question) => [...(question.default_answers ?? [])]);
}

/**
 * 将不属于预设选项的默认值放入自定义编辑框。
 *
 * @param questions 当前问题集合
 * @returns 每个问题对应的自定义输入初值
 */
function initialCustomDrafts(questions: QuestionPrompt[]): string[] {
  return questions.map((question) => {
    const optionValues = new Set(question.options.map((option) => option.value ?? option.label));
    return (question.default_answers ?? []).find((answer) => !optionValues.has(answer)) ?? "";
  });
}

/**
 * 返回提问卡片当前状态文案。
 *
 * @param status 卡片状态
 * @param active 当前轮次是否仍可交互
 * @param t 双语文本选择方法
 * @returns 用户可读状态文案
 */
function statusLabel(status: CardStatus, active: boolean, t: (en: string, zh: string) => string): string {
  if (status === "pending" && !active) return t("Questions ended", "提问已结束");
  return {
    pending: t("Your answer is required", "需要你的回答"),
    answered: t("Answered", "已回答"),
    cancelled: t("Cancelled", "已取消"),
    unavailable: t("Questions unavailable", "无法提问")
  }[status];
}

/**
 * 将提问响应转换为卡片状态。
 *
 * @param response 可选提问响应
 * @returns 对应卡片状态
 */
function responseStatus(response?: QuestionResponse): CardStatus {
  if (!response) return "pending";
  if (response.status === "answered") return "answered";
  if (response.status === "cancelled") return "cancelled";
  return "unavailable";
}

/**
 * 将已提交答案转换为卡片摘要。
 *
 * @param response 可选提问响应
 * @param t 双语文本选择方法
 * @returns 每个问题对应的摘要文本
 */
function summaryFromResponse(response: QuestionResponse | undefined, t: (en: string, zh: string) => string): string[] {
  if (!response || response.status !== "answered") return [];
  return response.data.map((item) => item.join(t(", ", "、")));
}
