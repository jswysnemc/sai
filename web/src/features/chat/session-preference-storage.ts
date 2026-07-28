import type { RunModelSelection, ThinkingLevel } from "../../api/contracts";

const GLOBAL_MODEL_KEY = "sai.chat-model";
const GLOBAL_THINKING_KEY = "sai.thinking-level";
const THINKING_LEVELS: ThinkingLevel[] = ["auto", "none", "low", "medium", "high", "xhigh", "max"];

/**
 * 【会话】【偏好存储】读取模型偏好；会话没有专属键时回退全局偏好。
 *
 * @param sessionId 可选会话 ID
 * @returns 合法模型选择；显式空值或无有效配置时返回 null
 */
export function readStoredChatModelSelection(sessionId?: string): RunModelSelection | null {
  try {
    const sessionValue = sessionId
      ? window.localStorage.getItem(modelStorageKey(sessionId))
      : null;
    const raw = sessionValue ?? window.localStorage.getItem(GLOBAL_MODEL_KEY);
    const value = JSON.parse(raw ?? "null") as Partial<RunModelSelection> | null;
    if (value?.providerId && value.model) {
      return { providerId: value.providerId, model: value.model };
    }
  } catch {
    return null;
  }
  return null;
}

/**
 * 【会话】【偏好存储】写入模型偏好；null 会建立会话专属空值，阻止回退旧的全局选择。
 *
 * @param sessionId 可选会话 ID
 * @param selection 模型选择或显式空值
 * @returns 无返回值
 */
export function writeStoredChatModelSelection(
  sessionId: string | undefined,
  selection: RunModelSelection | null
): void {
  try {
    window.localStorage.setItem(modelStorageKey(sessionId), JSON.stringify(selection));
  } catch {
    // 【会话】【偏好存储】浏览器禁用本地存储时仍允许会话继续运行
  }
}

/**
 * 【会话】【偏好存储】读取思考等级；会话没有专属键时回退全局偏好。
 *
 * @param sessionId 可选会话 ID
 * @returns 合法思考等级，未知值回退 auto
 */
export function readStoredThinkingLevel(sessionId?: string): ThinkingLevel {
  try {
    const stored = window.localStorage.getItem(thinkingStorageKey(sessionId))
      ?? (sessionId ? window.localStorage.getItem(GLOBAL_THINKING_KEY) : null);
    return normalizeThinkingLevel(stored);
  } catch {
    return "auto";
  }
}

/**
 * 【会话】【偏好存储】写入会话或全局思考等级。
 *
 * @param sessionId 可选会话 ID
 * @param level 思考等级
 * @returns 无返回值
 */
export function writeStoredThinkingLevel(
  sessionId: string | undefined,
  level: ThinkingLevel
): void {
  try {
    window.localStorage.setItem(thinkingStorageKey(sessionId), level);
  } catch {
    // 【会话】【偏好存储】浏览器禁用本地存储时仍允许会话继续运行
  }
}

/**
 * 【会话】【偏好存储】把未知配置值归一为支持的思考等级。
 *
 * @param value 待校验值
 * @returns 合法思考等级，未知值返回 auto
 */
export function normalizeThinkingLevel(value: unknown): ThinkingLevel {
  return typeof value === "string" && THINKING_LEVELS.includes(value as ThinkingLevel)
    ? value as ThinkingLevel
    : "auto";
}

/**
 * 【会话】【偏好存储】返回模型偏好的浏览器存储键。
 *
 * @param sessionId 可选会话 ID
 * @returns 会话专属键或全局键
 */
function modelStorageKey(sessionId?: string): string {
  return sessionId ? `${GLOBAL_MODEL_KEY}.${sessionId}` : GLOBAL_MODEL_KEY;
}

/**
 * 【会话】【偏好存储】返回思考等级偏好的浏览器存储键。
 *
 * @param sessionId 可选会话 ID
 * @returns 会话专属键或全局键
 */
function thinkingStorageKey(sessionId?: string): string {
  return sessionId ? `${GLOBAL_THINKING_KEY}.${sessionId}` : GLOBAL_THINKING_KEY;
}
