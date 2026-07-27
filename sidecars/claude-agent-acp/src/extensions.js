/** Sai 专属 ACP 扩展字段的命名空间。 */
export const SAI_EXTENSION_NAMESPACE = "_sai";

/**
 * 构造不会污染标准 ACP 字段的 Sai 扩展元数据。
 *
 * @param {Record<string, unknown>} fields 扩展字段
 * @returns {{_sai: Record<string, unknown>}} ACP 扩展对象
 */
export function saiExtensions(fields) {
  return { [SAI_EXTENSION_NAMESPACE]: fields };
}
