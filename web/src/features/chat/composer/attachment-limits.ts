export const MAX_IMAGE_ATTACHMENTS = 4;
export const MAX_IMAGE_ATTACHMENT_BYTES = 2 * 1024 * 1024;

export type AttachmentLimitViolation = "too_many" | "too_large" | null;

/**
 * 校验待加入图片是否满足数量和单文件尺寸限制。
 *
 * @param existingCount 已有附件数量
 * @param files 待加入文件尺寸
 * @returns 限制类型；满足限制时返回 null
 */
export function attachmentLimitViolation(
  existingCount: number,
  files: ArrayLike<Pick<File, "size">>
): AttachmentLimitViolation {
  if (existingCount + files.length > MAX_IMAGE_ATTACHMENTS) return "too_many";
  for (const file of Array.from(files)) {
    if (file.size > MAX_IMAGE_ATTACHMENT_BYTES) return "too_large";
  }
  return null;
}
