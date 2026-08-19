export const MAX_IMAGE_ATTACHMENTS = 4;

export type AttachmentLimitViolation = "too_many" | null;

/**
 * 校验待加入图片是否超过数量上限。
 *
 * 单张体积不在 SAI 侧限制，由上游模型接口拒绝过大的请求。
 *
 * @param existingCount 已有附件数量
 * @param files 待加入文件
 * @returns 超出数量时返回 too_many，否则返回 null
 */
export function attachmentLimitViolation(
  existingCount: number,
  files: ArrayLike<unknown>
): AttachmentLimitViolation {
  if (existingCount + files.length > MAX_IMAGE_ATTACHMENTS) return "too_many";
  return null;
}
