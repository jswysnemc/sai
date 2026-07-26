/** 双语文案取值函数。 */
export type Translate = (en: string, zh: string) => string;

/** 用量面板的视图标识。 */
export type UsageView = "overview" | "providers" | "models" | "logs";

/**
 * 按标识取双语文案。
 *
 * @param map 标识到中英文案的映射
 * @param key 标识
 * @param t 双语取值函数
 * @returns 对应文案，未命中时回落为标识本身
 */
function pick(map: Record<string, [string, string]>, key: string, t: Translate) {
  const pair = map[key] ?? [key, key];
  return t(pair[0], pair[1]);
}

/**
 * 时间范围文案。
 *
 * @param range 范围标识
 * @param t 双语取值函数
 * @returns 范围显示名
 */
export function rangeLabel(range: string, t: Translate) {
  return pick(
    {
      today: ["Today", "今天"],
      "1d": ["Last 24h", "近 24 小时"],
      "7d": ["Last 7 days", "近 7 天"],
      "30d": ["Last 30 days", "近 30 天"],
      "90d": ["Last 90 days", "近 90 天"],
      all: ["All time", "全部"],
    },
    range,
    t
  );
}

/**
 * 调用来源文案。
 *
 * @param source 来源标识
 * @param t 双语取值函数
 * @returns 来源显示名
 */
export function sourceLabel(source: string, t: Translate) {
  return pick(
    {
      all: ["All sources", "全部来源"],
      chat: ["Chat", "对话"],
      compaction: ["Compaction", "上下文压缩"],
      session_memory: ["Session memory", "会话记忆"],
    },
    source,
    t
  );
}

/**
 * 请求状态文案。
 *
 * @param status 状态标识
 * @param t 双语取值函数
 * @returns 状态显示名
 */
export function statusLabel(status: string, t: Translate) {
  return pick(
    {
      all: ["All statuses", "全部状态"],
      success: ["Success", "成功"],
      error: ["Error", "失败"],
      missing_usage: ["No usage", "无用量"],
    },
    status,
    t
  );
}

/**
 * 视图标签文案。
 *
 * @param view 视图标识
 * @param t 双语取值函数
 * @returns 视图显示名
 */
export function viewLabel(view: string, t: Translate) {
  return pick(
    {
      overview: ["Overview", "总览"],
      providers: ["Providers", "供应商"],
      models: ["Models", "模型"],
      logs: ["Logs", "日志"],
    },
    view,
    t
  );
}
