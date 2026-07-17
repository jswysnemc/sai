/** 跨路由保留的会话级输入草稿。 */
const drafts = new Map<string, string>();

/**
 * 读取指定会话的草稿。
 *
 * @param sessionId 会话 ID
 * @returns 草稿文本
 */
export function readComposerDraft(sessionId?: string | null): string {
  if (!sessionId) return "";
  return drafts.get(sessionId) ?? "";
}

/**
 * 写入指定会话的草稿。
 *
 * @param sessionId 会话 ID
 * @param value 草稿文本
 */
export function writeComposerDraft(sessionId: string | null | undefined, value: string): void {
  if (!sessionId) return;
  if (!value) {
    drafts.delete(sessionId);
    return;
  }
  drafts.set(sessionId, value);
}

/**
 * 清空指定会话的草稿。
 *
 * @param sessionId 会话 ID
 */
export function clearComposerDraft(sessionId?: string | null): void {
  if (!sessionId) return;
  drafts.delete(sessionId);
}
