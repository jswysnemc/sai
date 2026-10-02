import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, MessageSquareText } from "../../shared/ui/icons";
import type { PendingQuestion, QuestionResponse } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Collapse } from "../../shared/ui/collapse/collapse";
import { useI18n } from "../i18n/use-i18n";
import { isQuestionSatisfied, statusLabel } from "./question-card-state";
import { QuestionHistory } from "./question-history";
import { QuestionOptionPanel } from "./question-option-panel";
import { QuestionStepDots } from "./question-step-dots";
import { QuestionSummaryBar } from "./question-summary-bar";
import { useQuestionAnswers } from "./use-question-answers";
import "./question-request-card.css";
import { PlanReviewContent } from "./plan-review-content";

type QuestionRequestCardProps = {
  pending: PendingQuestion;
  response?: QuestionResponse;
  active?: boolean;
};

/**
 * 在助手消息流内渲染结构化提问卡片。
 *
 * 待回答时展开当前问题与选项；回答、取消或随运行结束后收缩为单行摘要条，
 * 点击摘要条只用于查看历史选择。
 *
 * @param props 待回答问题、可选响应结果和当前轮次状态
 * @returns 结构化提问卡片
 */
export function QuestionRequestCard({ pending, response, active = true }: QuestionRequestCardProps) {
  const { t } = useI18n();
  const questions = pending.request.questions;
  const controller = useQuestionAnswers(pending, response, t);
  const [historyOpen, setHistoryOpen] = useState(false);
  useEffect(() => setHistoryOpen(false), [pending.id, response]);

  const { status, step, answers, summary, submitting, error } = controller;
  const live = status === "pending" && active;
  const current = questions[step] ?? questions[0];
  const satisfied = questions.map((question, index) => isQuestionSatisfied(question, answers[index]));
  const label = statusLabel(status, active, t);
  // 1. 摘要条优先给出选择结果，没有结果时退回问题标题
  const summaryLine = summary.some(Boolean)
    ? summary.filter(Boolean).join(" · ")
    : questions.map((question) => question.header).join(" / ");

  return (
    <section className={`question-request-card is-${status}${live ? " is-live" : ""}`}>
      {live ? (
        <header className="question-request-head icon-label">
          <MessageSquareText size={14} className="question-status-icon" aria-hidden />
          <strong>{label}</strong>
          {current && <span className="question-request-topic">{current.header}</span>}
        </header>
      ) : (
        <QuestionSummaryBar status={status} label={label} summary={summaryLine} expanded={historyOpen} onToggle={() => setHistoryOpen((value) => !value)} />
      )}
      <Collapse open={live || historyOpen}>
        {pending.plan && <PlanReviewContent plan={pending.plan} />}
        {live && current ? (
          <div className="question-request-body">
            <QuestionOptionPanel
              question={current}
              selected={answers[step] ?? []}
              customDraft={controller.customDrafts[step] ?? ""}
              attachmentKey={`question:${pending.id}:${step}`}
              interactive={live && !submitting}
              onToggle={(value) => controller.toggleOption(step, value, Boolean(current.multiple), live)}
              onCustomDraft={(value) => controller.setCustomDraft(step, value)}
              onSaveCustom={(images) => controller.saveCustom(step, Boolean(current.multiple), live, images)}
              steps={<QuestionStepDots count={questions.length} current={step} satisfied={satisfied} onSelect={controller.setStep} t={t} />}
              t={t}
            />
            <div className="question-request-actions">
              {questions.length > 1 && (
                <div className="question-pager">
                  <Button
                    variant="ghost"
                    size="icon"
                    className="question-pager-button"
                    disabled={submitting || step <= 0}
                    aria-label={t("Previous question", "上一题")}
                    onClick={() => controller.setStep(step - 1)}
                  >
                    <ChevronLeft size={14} />
                  </Button>
                  <span className="question-pager-index">{step + 1}/{questions.length}</span>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="question-pager-button"
                    disabled={submitting || step >= questions.length - 1}
                    aria-label={t("Next question", "下一题")}
                    onClick={() => controller.setStep(step + 1)}
                  >
                    <ChevronRight size={14} />
                  </Button>
                </div>
              )}
              {error && <span className="question-request-error" role="alert">{error.message}</span>}
              <Button variant="ghost" size="small" disabled={submitting} onClick={() => void controller.cancel()}>
                {t("Cancel", "取消")}
              </Button>
              <Button variant="primary" size="small" disabled={submitting || !satisfied.every(Boolean)} onClick={() => void controller.submit()}>
                {submitting ? t("Submitting", "提交中") : pending.plan && answers[0]?.includes("Approve and implement") ? t("Execute plan", "执行计划") : t("Confirm", "确认")}
              </Button>
            </div>
          </div>
        ) : (
          <QuestionHistory questions={questions} summary={summary} images={controller.imageUrls} t={t} />
        )}
      </Collapse>
    </section>
  );
}
