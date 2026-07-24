/** 跨路由保留的会话级图片附件草稿。 */
export type ComposerAttachmentDraft = {
  id: number;
  name: string;
  dataUrl: string;
};

const attachmentDrafts = new Map<string, ComposerAttachmentDraft[]>();

/**
 * 读取指定会话的图片附件草稿。
 *
 * @param sessionId 会话 ID
 * @returns 附件草稿列表
 */
export function readComposerAttachmentDraft(sessionId?: string | null): ComposerAttachmentDraft[] {
  if (!sessionId) return [];
  return attachmentDrafts.get(sessionId) ?? [];
}

/**
 * 写入指定会话的图片附件草稿。
 *
 * @param sessionId 会话 ID
 * @param items 附件列表
 * @returns 无返回值
 */
export function writeComposerAttachmentDraft(
  sessionId: string | null | undefined,
  items: ComposerAttachmentDraft[]
): void {
  if (!sessionId) return;
  if (items.length === 0) {
    attachmentDrafts.delete(sessionId);
    return;
  }
  attachmentDrafts.set(sessionId, items.map((item) => ({ ...item })));
}

/**
 * 清空指定会话的图片附件草稿。
 *
 * @param sessionId 会话 ID
 * @returns 无返回值
 */
export function clearComposerAttachmentDraft(sessionId?: string | null): void {
  if (!sessionId) return;
  attachmentDrafts.delete(sessionId);
}
