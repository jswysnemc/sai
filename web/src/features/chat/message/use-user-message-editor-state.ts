import { useState } from "react";
import { useI18n } from "../../i18n/use-i18n";
import {
  attachmentLimitViolation,
  MAX_IMAGE_ATTACHMENTS,
  MAX_IMAGE_ATTACHMENT_BYTES
} from "../composer/attachment-limits";
import { readImageAsDataUrl } from "../composer/read-image-as-data-url";
import {
  appendEditableImages,
  isEditorDirty,
  isEditorSubmittable,
  toEditableImages,
  type EditableImage
} from "./user-message-editor-model";

/**
 * 管理用户消息编辑态的正文与图片。
 *
 * 原消息的图片会作为初值带入，因此重新发送时不会丢图；新增图片走与输入区
 * 相同的数量与大小限制，避免两处规则不一致。
 *
 * @param initialContent 原消息正文
 * @param initialImageUrls 原消息图片地址
 * @returns 编辑态数据与操作方法
 */
export function useUserMessageEditorState(initialContent: string, initialImageUrls: string[]) {
  const { t } = useI18n();
  const [content, setContent] = useState(initialContent);
  const [images, setImages] = useState<EditableImage[]>(() => toEditableImages(initialImageUrls));
  const [sequence, setSequence] = useState(initialImageUrls.length);
  const [error, setError] = useState<string | null>(null);

  /**
   * 校验并追加图片文件。
   *
   * @param files 待加入的文件，非图片会被忽略
   * @returns 无返回值
   */
  const addFiles = async (files: File[]) => {
    const picked = files.filter((file) => file.type.startsWith("image/"));
    if (picked.length === 0) return;
    // 1. 数量与单张大小沿用输入区的限制
    const violation = attachmentLimitViolation(images.length, picked);
    if (violation === "too_many") {
      setError(t(
        `Attach at most ${MAX_IMAGE_ATTACHMENTS} images`,
        `最多添加 ${MAX_IMAGE_ATTACHMENTS} 张图片`
      ));
      return;
    }
    if (violation === "too_large") {
      const megabytes = MAX_IMAGE_ATTACHMENT_BYTES / 1024 / 1024;
      setError(t(
        `Each image must be ${megabytes} MiB or smaller`,
        `每张图片不能超过 ${megabytes} MiB`
      ));
      return;
    }
    // 2. 读取为 data URL 后按当前顺序追加
    try {
      const loaded = await Promise.all(picked.map((file) =>
        readImageAsDataUrl(file, () => new Error(t("Failed to read image", "读取图片失败")))
      ));
      setError(null);
      const next = appendEditableImages(images, loaded, sequence);
      setImages(next.images);
      setSequence(next.sequence);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : t("Failed to read image", "读取图片失败"));
    }
  };

  /**
   * 移除一张图片。
   *
   * @param id 图片本地标识
   * @returns 无返回值
   */
  const removeImage = (id: number) => {
    setImages((current) => current.filter((image) => image.id !== id));
    setError(null);
  };

  return {
    content,
    setContent,
    images,
    error,
    addFiles,
    removeImage,
    /** 内容或图片相对原消息有改动 */
    dirty: isEditorDirty(content, images, initialContent, initialImageUrls),
    /** 至少有正文或图片，可以提交 */
    submittable: isEditorSubmittable(content, images),
    imageUrls: images.map((image) => image.dataUrl)
  };
}
