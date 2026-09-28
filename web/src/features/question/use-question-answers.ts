import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { LocalizedError, toDisplayError } from "../../api/api-error";
import type { PendingQuestion, QuestionAnswers, QuestionResponse } from "../../api/contracts";
import {
  firstUnanswered,
  initialAnswers,
  initialCustomDrafts,
  responseStatus,
  summarizeAnswers,
  summaryFromResponse,
  type CardStatus,
  type Translate
} from "./question-card-state";

/** 提问卡片的答题状态与操作。 */
export type QuestionAnswersController = {
  status: CardStatus;
  step: number;
  answers: QuestionAnswers;
  customDrafts: string[];
  summary: string[];
  imageUrls: string[][];
  submitting: boolean;
  error: Error | null;
  setStep: (step: number) => void;
  setCustomDraft: (step: number, value: string) => void;
  toggleOption: (step: number, value: string, multiple: boolean, interactive: boolean) => void;
  saveCustom: (step: number, multiple: boolean, interactive: boolean, images?: string[]) => void;
  submit: (override?: QuestionAnswers, imagesOverride?: string[][]) => Promise<void>;
  cancel: () => Promise<void>;
};

/**
 * 管理结构化提问的答案、步进位置与提交流程。
 *
 * 单选问题选中即推进：跳到第一个仍需作答的问题，全部完成则直接提交。
 *
 * @param pending 待回答提问
 * @param response 服务端已有的响应结果
 * @param t 双语文案取值函数
 * @returns 答题状态与操作
 */
export function useQuestionAnswers(pending: PendingQuestion, response: QuestionResponse | undefined, t: Translate): QuestionAnswersController {
  const questions = pending.request.questions;
  const [status, setStatus] = useState<CardStatus>(() => responseStatus(response));
  const [step, setStep] = useState(0);
  const [answers, setAnswers] = useState<QuestionAnswers>(() => initialAnswers(questions));
  const [customDrafts, setCustomDrafts] = useState<string[]>(() => initialCustomDrafts(questions));
  const [summary, setSummary] = useState<string[]>(() => summaryFromResponse(response, t));
  const [imageUrls, setImageUrls] = useState<string[][]>(() => response?.status === "answered_with_images" ? response.data.image_urls : questions.map(() => []));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  // 1. 提问或响应变化时整体重置，避免沿用上一条提问的草稿
  useEffect(() => {
    setStatus(responseStatus(response));
    setStep(0);
    setAnswers(initialAnswers(questions));
    setCustomDrafts(initialCustomDrafts(questions));
    setSummary(summaryFromResponse(response, t));
    setImageUrls(response?.status === "answered_with_images" ? response.data.image_urls : questions.map(() => []));
    setSubmitting(false);
    setError(null);
  }, [pending.id, questions, response, t]);

  /**
   * 校验并提交全部问题答案。
   *
   * @param override 单选自动提交时传入的最新答案，绕开状态更新延迟
   * @returns 提交完成后的 Promise
   */
  const submit = async (override?: QuestionAnswers, imagesOverride?: string[][]): Promise<void> => {
    const finalAnswers = override ?? answers;
    if (firstUnanswered(questions, finalAnswers) !== -1) {
      setError(new LocalizedError("Answer every question first", "请先回答所有问题"));
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await api.questions.answer(pending.id, finalAnswers, imagesOverride ?? imageUrls);
      setStatus("answered");
      setSummary(summarizeAnswers(finalAnswers, t));
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
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to cancel questions", "取消提问失败"));
    } finally {
      setSubmitting(false);
    }
  };

  /**
   * 单选作答后的推进：跳到第一个未回答的问题，全部完成则自动提交。
   *
   * @param next 最新的答案集合
   * @returns 无返回值
   */
  const advanceOrSubmit = (next: QuestionAnswers, nextImages: string[][]): void => {
    const target = firstUnanswered(questions, next);
    if (target === -1) void submit(next, nextImages);
    else if (target !== step) setStep(target);
  };

  /**
   * 写入指定问题的回答并按需推进。
   *
   * @param index 问题索引
   * @param value 新回答
   * @param advance 是否执行单选推进
   * @returns 无返回值
   */
  const commit = (index: number, value: string[], advance: boolean, images: string[] = []): void => {
    const next = answers.map((item, position) => (position === index ? value : [...item]));
    setAnswers(next);
    const nextImages = imageUrls.map((item, position) => position === index ? images : item);
    setImageUrls(nextImages);
    if (advance) advanceOrSubmit(next, nextImages);
  };

  const toggleOption = (index: number, value: string, multiple: boolean, interactive: boolean): void => {
    const selected = answers[index] ?? [];
    // 1. 多选切换勾选，单选直接替换
    const nextValue = multiple
      ? selected.includes(value) ? selected.filter((item) => item !== value) : [...selected, value]
      : [value];
    commit(index, nextValue, !multiple && interactive, multiple ? imageUrls[index] : []);
  };

  const saveCustom = (index: number, multiple: boolean, interactive: boolean, images: string[] = []): void => {
    const value = (customDrafts[index] ?? "").trim() || (images.length ? t("See attached images", "请查看所附图片") : "");
    if (!value) return;
    const selected = answers[index] ?? [];
    // 1. 多选追加且去重，单选替换为自定义回答
    const nextValue = multiple ? (selected.includes(value) ? selected : [...selected, value]) : [value];
    commit(index, nextValue, !multiple && interactive, images);
  };

  const setCustomDraft = (index: number, value: string): void => {
    setCustomDrafts((prev) => prev.map((item, position) => (position === index ? value : item)));
  };

  return { status, step, answers, customDrafts, summary, imageUrls, submitting, error, setStep, setCustomDraft, toggleOption, saveCustom, submit, cancel };
}
