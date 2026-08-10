import type { ProviderConfig } from "../../../api/contracts";

/**
 * 在供应商列表中定位选中项。
 *
 * 找不到时返回 -1 而非回落到首项：回落会让后续的按索引写入静默打到
 * 别的供应商身上，表现为"自动切换并丢失编辑内容"。
 *
 * @param providers 供应商列表
 * @param selectedId 当前选中标识
 * @returns 选中项索引；不存在时为 -1
 */
export function findProviderIndex(
  providers: readonly ProviderConfig[],
  selectedId: string
): number {
  return providers.findIndex((provider) => provider.id === selectedId);
}

/**
 * 为新增供应商生成不重复的标识。
 *
 * @param providers 现有供应商列表
 * @returns 未被占用的标识
 */
export function nextProviderId(providers: readonly ProviderConfig[]): string {
  let suffix = 1;
  let id = "provider";
  while (providers.some((provider) => provider.id === id)) {
    suffix += 1;
    id = `provider-${suffix}`;
  }
  return id;
}

/**
 * 校验供应商标识改名是否可用。
 *
 * 标识是配置文件里的主键，重名会让两条记录互相覆盖；留空则无法再被引用。
 *
 * @param providers 供应商列表
 * @param index 正在改名的供应商索引
 * @param nextId 目标标识
 * @returns 冲突原因；可用时为空
 */
export function providerIdConflict(
  providers: readonly ProviderConfig[],
  index: number,
  nextId: string
): "empty" | "duplicate" | "" {
  if (!nextId.trim()) return "empty";
  const duplicated = providers.some(
    (provider, providerIndex) => providerIndex !== index && provider.id === nextId
  );
  return duplicated ? "duplicate" : "";
}
