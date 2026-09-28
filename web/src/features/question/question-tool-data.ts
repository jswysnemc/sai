import type { QuestionPrompt } from "../../api/contracts";
import { parseJsonRecord } from "../chat/tool-renderers/tool-data";

/**
 * 解析提问工具的参数和历史回答，避免把协议字段直接展示为 JSON。
 * @param argumentsText 工具参数；output 工具结果
 * @returns 问题、选项、回答与附件计数；非法参数返回 null
 */
export function parseQuestionTool(argumentsText: string, output: string) {
  const request = parseJsonRecord(argumentsText);
  if (!Array.isArray(request?.questions)) return null;
  const result = parseJsonRecord(output);
  const answers = Array.isArray(result?.answers) ? result.answers : [];
  const questions = request.questions.flatMap((entry, index) => {
    if (!entry || typeof entry !== "object") return [];
    const value = entry as Record<string, unknown>;
    if (typeof value.question !== "string") return [];
    const options = Array.isArray(value.options) ? value.options.flatMap((item) => {
      if (!item || typeof item !== "object" || typeof item.label !== "string") return [];
      return [{ label: item.label, description: typeof item.description === "string" ? item.description : "", value: typeof item.value === "string" ? item.value : undefined }];
    }) : [];
    const question: QuestionPrompt = {
      header: typeof value.header === "string" ? value.header : "",
      question: value.question, options, multiple: value.multiple === true, custom: false
    };
    const answer = answers[index] as Record<string, unknown> | undefined;
    const raw = answer?.answer;
    const selected = Array.isArray(raw) ? raw.filter((item): item is string => typeof item === "string") : typeof raw === "string" && raw ? [raw] : [];
    return [{ question, selected, imageCount: typeof answer?.image_count === "number" ? answer.image_count : 0 }];
  });
  return questions.length ? { questions, status: result?.status, reason: typeof result?.reason === "string" ? result.reason : "" } : null;
}
