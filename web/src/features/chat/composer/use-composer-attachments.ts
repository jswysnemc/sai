import { useRef, useState } from "react";

export type ComposerAttachment = {
  id: number;
  name: string;
  dataUrl: string;
};

/**
 * 管理输入区图片、token 插入和文本同步。
 *
 * @param value 当前输入文本
 * @param onValueChange 输入文本更新回调
 * @returns 图片附件状态和操作方法
 */
export function useComposerAttachments() {
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([]);
  const sequence = useRef(0);

  /**
   * 读取并加入一组独立图片附件。
   *
   * @param files 图片文件
   * @param selectionStart 插入选区起点
   * @param selectionEnd 插入选区终点
   */
  const addFiles = async (files: File[], selectionStart: number, selectionEnd: number): Promise<number | undefined> => {
    const images = files.filter((file) => file.type.startsWith("image/"));
    if (images.length === 0) return undefined;
    const loaded = await Promise.all(images.map(async (file) => ({
      name: file.name || `粘贴图片_${Date.now()}.png`,
      dataUrl: await readFileAsDataUrl(file)
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
   */
  const removeAttachment = (id: number) => {
    setAttachments((items) => items.filter((item) => item.id !== id));
  };

  /**
   * 根据输入文本中仍存在的 token 清理附件。
   *
   * @param text 最新输入文本
   */
  /** 清空全部附件。 */
  const clearAttachments = () => setAttachments([]);

  /**
   * 恢复指定附件列表。
   *
   * @param items 要恢复的附件
   */
  const restoreAttachments = (items: ComposerAttachment[]) => setAttachments(items);

  return { attachments, addFiles, removeAttachment, clearAttachments, restoreAttachments };
}

/**
 * 将图片文件读取为 data URL。
 *
 * @param file 图片文件
 * @returns 图片 data URL
 */
function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("读取图片失败"));
    reader.readAsDataURL(file);
  });
}
