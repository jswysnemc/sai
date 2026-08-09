/**
 * 敏感值编辑的 sentinel 合并算法。
 *
 * 服务端向前端下发敏感数组时用占位符隐藏已保存的值；用户编辑可见
 * 条目后，提交前必须把占位符按原索引合回去，服务端才能恢复对应密钥。
 * 此前 structured-config-fields 与 web-search 各写了一份同算法，这里合一。
 */

/**
 * 合并编辑后的可见条目与服务端占位符。
 *
 * 步骤:
 * 1. 逐个用可见条目替换非占位槽位，占位槽位保持原索引
 * 2. 多出的可见条目追加到末尾
 * 3. 移除末尾无占位意义的空槽（占位符之前的空槽必须保留）
 *
 * 参数:
 * - `current`: 当前脱敏数组（含占位符）
 * - `visible`: 用户编辑后的可见条目
 * - `secretSentinel`: 服务端敏感字段占位符；为空时直接采用可见条目
 *
 * 返回:
 * - 可安全提交并由服务端恢复隐藏值的数组
 */
export function mergeSecretValues(
  current: string[],
  visible: string[],
  secretSentinel: string
): string[] {
  if (!secretSentinel) return [...visible];
  let visibleIndex = 0;
  const merged = current.map((item) => {
    if (item === secretSentinel) return item;
    const replacement = visible[visibleIndex];
    visibleIndex += 1;
    return replacement ?? "";
  });
  merged.push(...visible.slice(visibleIndex));
  while (merged.at(-1) === "") merged.pop();
  return merged;
}

/**
 * 从多行/逗号分隔文本解析可见条目后合并占位符。
 *
 * 参数:
 * - `current`: 当前脱敏数组（含占位符）
 * - `text`: textarea 原始文本
 * - `secretSentinel`: 服务端敏感字段占位符
 *
 * 返回:
 * - 合并后的可提交数组
 */
export function mergeSecretText(
  current: string[],
  text: string,
  secretSentinel: string
): string[] {
  const visible = text.split(/[\n\r,]/).map((item) => item.trim()).filter(Boolean);
  return mergeSecretValues(current, visible, secretSentinel);
}
