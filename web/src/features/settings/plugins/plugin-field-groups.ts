/** 插件配置字段的分组标识 */
export type PluginFieldGroupId = "credentials" | "endpoints" | "limits" | "switches" | "other";

/** 一组插件配置字段 */
export type PluginFieldGroup = {
  id: PluginFieldGroupId;
  entries: [string, unknown][];
};

/** 分组的固定展示顺序：先接得通，再调得动，最后是细节开关 */
const GROUP_ORDER: PluginFieldGroupId[] = ["credentials", "endpoints", "limits", "switches", "other"];

/**
 * 判断字段是否为插件总开关。
 *
 * 总开关决定整个插件是否生效，单独提到顶部，不参与分组。
 *
 * @param name 字段名
 * @returns 是否为总开关
 */
export function isPluginEnabledField(name: string): boolean {
  return name === "enabled";
}

/**
 * 按语义把插件配置字段分组。
 *
 * 改造前所有字段平铺在一个网格里，凭据、地址、数值与开关混排，
 * 高度参差且看不出哪些是"必须先填"的。分组后阅读顺序与配置顺序一致。
 *
 * @param config 插件配置对象
 * @returns 非空分组列表，按固定顺序排列
 */
export function groupPluginFields(config: Record<string, unknown>): PluginFieldGroup[] {
  const buckets = new Map<PluginFieldGroupId, [string, unknown][]>();
  for (const [name, value] of Object.entries(config)) {
    if (isPluginEnabledField(name)) continue;
    const id = classifyPluginField(name, value);
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
 * 判定单个字段所属分组。
 *
 * 名称匹配优先于类型：`timeout_seconds` 是数值也是限额，
 * 而 `safe_search` 是布尔且没有名称特征，落到开关组。
 *
 * @param name 字段名
 * @param value 字段值
 * @returns 分组标识
 */
export function classifyPluginField(name: string, value: unknown): PluginFieldGroupId {
  if (/(api_key|apikey|token|secret|password|credential)/.test(name)) return "credentials";
  if (/(url|endpoint|host|base|dir|path|proxy)/.test(name)) return "endpoints";
  if (/(max_|min_|_count|_limit|timeout|_seconds|_mb|depth|rounds|steps|retries|concurrency)/.test(name)) {
    return "limits";
  }
  if (typeof value === "boolean") return "switches";
  return "other";
}
