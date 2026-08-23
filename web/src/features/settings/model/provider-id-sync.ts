/**
 * 判断供应商标识是否仍跟随显示名称。
 *
 * 新建时的 provider / provider-N，或已经与名称相同的标识，都视为可同步。
 *
 * @param id 当前标识
 * @param displayName 当前显示名称
 * @returns 名称变更时应同步标识
 */
export function providerIdFollowsName(id: string, displayName: string): boolean {
  if (id === displayName.trim()) return true;
  return /^provider(?:-\d+)?$/.test(id);
}

/**
 * 由显示名称得到建议标识，并检测是否与其它供应商冲突。
 *
 * @param displayName 显示名称
 * @param providers 全部供应商
 * @param currentIndex 正在编辑的供应商索引
 * @returns 建议标识与是否冲突
 */
export function suggestedProviderId(
  displayName: string,
  providers: readonly { id: string }[],
  currentIndex: number
): { id: string; conflict: boolean } {
  const id = displayName.trim();
  if (!id) return { id: "", conflict: false };
  const conflict = providers.some((provider, index) => index !== currentIndex && provider.id === id);
  return { id, conflict };
}
