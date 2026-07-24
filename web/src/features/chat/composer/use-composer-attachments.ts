import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/use-i18n";
import {
  attachmentLimitViolation,
  MAX_IMAGE_ATTACHMENTS,
  MAX_IMAGE_ATTACHMENT_BYTES
} from "./attachment-limits";
import {
  clearComposerAttachmentDraft,
  readComposerAttachmentDraft,
  writeComposerAttachmentDraft,
  type ComposerAttachmentDraft
} from "./composer-attachments-draft";

export type ComposerAttachment = ComposerAttachmentDraft;

/**
 * 管理输入区图片、token 插入和文本同步。
 *
 * @param sessionId 当前会话标识；切换会话时恢复对应附件草稿
 * @returns 图片附件状态和操作方法
 */
export function useComposerAttachments(sessionId?: string | null) {
  const { t } = useI18n();
  const [attachments, setAttachments] = useState<ComposerAttachment[]>(() => readComposerAttachmentDraft(sessionId));
  const sequence = useRef(0);
  const sessionRef = useRef(sessionId);

  // 1. 切换会话时恢复草稿；同一会话内同步写入跨路由缓存
  useEffect(() => {
    if (sessionRef.current !== sessionId) {
      sessionRef.current = sessionId;
      const restored = readComposerAttachmentDraft(sessionId);
      sequence.current = restored.reduce((max, item) => Math.max(max, item.id), 0);
      setAttachments(restored);
      return;
    }
    writeComposerAttachmentDraft(sessionId, attachments);
  }, [attachments, sessionId]);

  /**
   * 读取并加入一组独立图片附件。
   *
   * @param files 图片文件
   * @param selectionStart 插入选区起点
   * @param selectionEnd 插入选区终点
   * @returns 插入后的光标位置
   */
  const addFiles = async (files: File[], selectionStart: number, selectionEnd: number): Promise<number | undefined> => {
    const images = files.filter((file) => file.type.startsWith("image/"));
    if (images.length === 0) return undefined;
    const violation = attachmentLimitViolation(attachments.length, images);
    if (violation === "too_many") {
      throw new Error(t(
        `Attach at most ${MAX_IMAGE_ATTACHMENTS} images`,
        `最多添加 ${MAX_IMAGE_ATTACHMENTS} 张图片`
      ));
    }
    if (violation === "too_large") {
      const megabytes = MAX_IMAGE_ATTACHMENT_BYTES / 1024 / 1024;
      throw new Error(t(
        `Each image must be ${megabytes} MiB or smaller`,
        `每张图片不能超过 ${megabytes} MiB`
      ));
    }
    const loaded = await Promise.all(images.map(async (file) => ({
      name: file.name || t(`pasted-image_${Date.now()}.png`, `粘贴图片_${Date.now()}.png`),
      dataUrl: await readFileAsDataUrl(file, t)
    })));
    const nextAttachments = [...attachments];
    for (const image of loaded) {
      sequence.current += 1;
      nextAttachments.push({ id: sequence.current, ...image });
    }
    setAttachments(nextAttachments);
    return selectionStart;
  };

  /**
   * 删除指定附件。
   *
   * @param id 附件标识
   * @returns 无返回值
   */
  const removeAttachment = (id: number) => {
    setAttachments((items) => items.filter((item) => item.id !== id));
  };

  /** 清空全部附件。 */
  const clearAttachments = () => {
    setAttachments([]);
    clearComposerAttachmentDraft(sessionId);
  };

  /**
   * 恢复指定附件列表。
   *
   * @param items 要恢复的附件
   * @returns 无返回值
   */
  const restoreAttachments = (items: ComposerAttachment[]) => {
    sequence.current = items.reduce((max, item) => Math.max(max, item.id), sequence.current);
    setAttachments(items);
  };

  return { attachments, addFiles, removeAttachment, clearAttachments, restoreAttachments };
}

/**
 * 将图片文件读取为 data URL。
 *
 * @param file 图片文件
 * @param t 双语文本选择方法
 * @returns 图片 data URL
 */
function readFileAsDataUrl(file: File, t: (en: string, zh: string) => string): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error(t("Failed to read image", "读取图片失败")));
    reader.readAsDataURL(file);
  });
}
