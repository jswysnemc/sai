/** CLI 助手工具配置字段的分组标识。 */
export type CliToolFieldGroupId = "credentials" | "endpoints" | "limits" | "switches" | "other";

/** 一组 CLI 助手工具配置字段。 */
export type CliToolFieldGroup = {
  id: CliToolFieldGroupId;
  entries: [string, unknown][];
};

const GROUP_ORDER: CliToolFieldGroupId[] = ["credentials", "endpoints", "limits", "switches", "other"];

/**
 * 判断字段是否为工具总开关。
 *
 * @param name 字段名称
 * @returns 字段为 enabled 时返回 true
 */
export function isCliToolEnabledField(name: string): boolean {
  return name === "enabled";
}

/**
 * 按配置用途对 CLI 助手工具字段分组。
 *
 * @param config 单个工具配置
 * @returns 按固定顺序排列的非空字段分组
 */
export function groupCliToolFields(config: Record<string, unknown>): CliToolFieldGroup[] {
  const buckets = new Map<CliToolFieldGroupId, [string, unknown][]>();
  for (const [name, value] of Object.entries(config)) {
    if (isCliToolEnabledField(name)) continue;
    const id = classifyCliToolField(name, value);
    const bucket = buckets.get(id) ?? [];
    bucket.push([name, value]);
    buckets.set(id, bucket);
  }
  return GROUP_ORDER.filter((id) => (buckets.get(id)?.length ?? 0) > 0).map((id) => ({
    id,
    entries: buckets.get(id) ?? []
  }));
}

/**
 * 判断单个工具字段所属分组。
 *
 * @param name 字段名称
 * @param value 字段值
 * @returns 字段分组标识
 */
export function classifyCliToolField(name: string, value: unknown): CliToolFieldGroupId {
  if (/(api_key|apikey|token|secret|password|credential)/.test(name)) return "credentials";
  if (/(url|endpoint|host|base|dir|path|proxy)/.test(name)) return "endpoints";
  if (/(max_|min_|_count|_limit|timeout|_seconds|_mb|depth|rounds|steps|retries|concurrency)/.test(name)) {
    return "limits";
  }
  if (typeof value === "boolean") return "switches";
  return "other";
}
