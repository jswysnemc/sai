import type { ProviderConfig } from "../../../api/contracts";

/**
 * 判断供应商是否启用。
 *
 * 字段缺省视为启用：旧配置文件里没有这一项，默认关闭会让升级后
 * 所有供应商一起消失。
 *
 * @param provider 供应商配置
 * @returns 启用时返回 true
 */
export function isProviderEnabled(provider: Pick<ProviderConfig, "enabled">): boolean {
  return provider.enabled !== false;
}

/**
 * 过滤出启用的供应商。
 *
 * @param providers 供应商列表
 * @returns 仅含启用项的列表
 */
export function enabledProviders<T extends Pick<ProviderConfig, "enabled">>(
  providers: readonly T[]
): T[] {
  return providers.filter(isProviderEnabled);
}

/**
 * 把供应商列表按启用状态分区。
 *
 * 停用项在设置页折叠到独立分区，既不干扰常用列表，又保留改回启用的入口。
 *
 * @param providers 供应商列表
 * @returns 启用与停用两组，各自保持原有顺序
 */
export function partitionByEnablement<T extends Pick<ProviderConfig, "enabled">>(
  providers: readonly T[]
): { enabled: T[]; disabled: T[] } {
  const enabled: T[] = [];
  const disabled: T[] = [];
  for (const provider of providers) {
    if (isProviderEnabled(provider)) {
      enabled.push(provider);
    } else {
      disabled.push(provider);
    }
  }
  return { enabled, disabled };
}

/**
 * 计算停用某个供应商后应当生效的当前供应商。
 *
 * 停用正在使用的供应商却不转移，会让后续请求全部落在一个已停用的
 * 配置上；这里回落到第一个仍然启用的供应商。
 *
 * @param providers 停用操作生效后的供应商列表
 * @param activeProvider 当前供应商标识
 * @returns 应当生效的当前供应商标识；无可用项时为空串
 */
export function nextActiveProvider(
  providers: readonly ProviderConfig[],
  activeProvider: string
): string {
  const current = providers.find((provider) => provider.id === activeProvider);
  if (current && isProviderEnabled(current)) return activeProvider;
  return enabledProviders(providers)[0]?.id ?? "";
}
