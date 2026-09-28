/** 字段锚点的元素标识前缀，搜索定位与字段组件共用。 */
export const FIELD_ANCHOR_PREFIX = "settings-field-";

/**
 * 把字段锚点转换为合法的元素标识。
 *
 * @param anchor 字段锚点，例如 runtime.context.compaction_ratio
 * @returns 可用于 id 属性与 getElementById 的标识
 */
export function fieldAnchorId(anchor: string): string {
  return FIELD_ANCHOR_PREFIX + anchor.replace(/[^a-zA-Z0-9_-]/g, "-");
}
