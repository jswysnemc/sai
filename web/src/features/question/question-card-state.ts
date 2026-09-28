import type { QuestionAnswers, QuestionPrompt, QuestionResponse } from "../../api/contracts";

/** 双语文案取值函数。 */
export type Translate = (en: string, zh: string) => string;

/** 提问卡片的生命周期状态。 */
export type CardStatus = "pending" | "answered" | "cancelled" | "unavailable";

/**
 * 从问题默认值构造初始回答。
 *
 * @param questions 当前问题集合
 * @returns 每个问题对应的初始回答
 */
export function initialAnswers(questions: QuestionPrompt[]): QuestionAnswers {
  return questions.map((question) => [...(question.default_answers ?? [])]);
}

/**
 * 将不属于预设选项的默认值放入自定义编辑框。
 *
 * @param questions 当前问题集合
 * @returns 每个问题对应的自定义输入初值
 */
export function initialCustomDrafts(questions: QuestionPrompt[]): string[] {
  return questions.map((question) => {
    const optionValues = new Set(question.options.map((option) => option.value ?? option.label));
    return (question.default_answers ?? []).find((answer) => !optionValues.has(answer)) ?? "";
  });
}

/**
 * 判断单个问题是否已满足作答要求（选填问题视为已满足）。
 *
 * @param question 问题定义
 * @param answer 当前回答
 * @returns 是否已满足
 */
export function isQuestionSatisfied(question: QuestionPrompt | undefined, answer: string[] | undefined): boolean {
  return question?.required === false || (answer?.length ?? 0) > 0;
}

/**
 * 找到第一个仍需作答的问题。
 *
 * @param questions 问题集合
 * @param answers 当前回答集合
 * @returns 问题索引；全部满足时返回 -1
 */
export function firstUnanswered(questions: QuestionPrompt[], answers: QuestionAnswers): number {
  return questions.findIndex((question, index) => !isQuestionSatisfied(question, answers[index]));
}

/**
 * 将提问响应转换为卡片状态。
 *
 * @param response 可选提问响应
 * @returns 对应卡片状态
 */
export function responseStatus(response?: QuestionResponse): CardStatus {
  if (!response) return "pending";
  if (response.status === "answered" || response.status === "answered_with_images") return "answered";
  if (response.status === "cancelled") return "cancelled";
  return "unavailable";
}

/**
 * 将已提交答案转换为每个问题的摘要文本。
 *
 * @param answers 已提交答案
 * @param t 双语文案取值函数
 * @returns 每个问题对应的摘要文本
 */
export function summarizeAnswers(answers: QuestionAnswers, t: Translate): string[] {
  return answers.map((item) => item.join(t(", ", "、")));
}

/**
 * 从服务端响应恢复答案摘要。
 *
 * @param response 可选提问响应
 * @param t 双语文案取值函数
 * @returns 每个问题对应的摘要；未回答时为空数组
 */
export function summaryFromResponse(response: QuestionResponse | undefined, t: Translate): string[] {
  if (response?.status === "answered_with_images") return summarizeAnswers(response.data.answers, t);
  if (!response || response.status !== "answered") return [];
  return summarizeAnswers(response.data, t);
}

/**
 * 返回提问卡片当前状态文案。
 *
 * @param status 卡片状态
 * @param active 当前轮次是否仍可交互
 * @param t 双语文案取值函数
 * @returns 用户可读状态文案
 */
export function statusLabel(status: CardStatus, active: boolean, t: Translate): string {
  if (status === "pending" && !active) return t("Questions ended", "提问已结束");
  return {
    pending: t("Your answer is required", "需要你的回答"),
    answered: t("Answered", "已回答"),
    cancelled: t("Cancelled", "已取消"),
    unavailable: t("Questions unavailable", "无法提问")
  }[status];
}
