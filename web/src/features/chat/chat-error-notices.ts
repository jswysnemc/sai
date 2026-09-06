import { errorDetailForDisplay } from "./message/run-error-notice";

type ChatErrorNotice = { key: string; message: string; detail: string };

/**
 * 合并时间线、模型和操作错误，避免输入区重复显示同一问题。
 * @param errors 以来源标识为键的错误集合
 * @returns 保持来源顺序的去重错误提示
 */
export function collectChatErrorNotices(errors: Record<string, Error | null>): ChatErrorNotice[] {
  const seen = new Set<string>();
  return Object.entries(errors).flatMap(([key, error]) => {
    if (!error) return [];
    const detail = errorDetailForDisplay(error);
    const signature = `${error.message}\n${detail}`;
    if (seen.has(signature)) return [];
    seen.add(signature);
    return [{ key, message: error.message, detail }];
  });
}
