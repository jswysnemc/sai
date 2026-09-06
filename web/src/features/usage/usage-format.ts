import { formatTokenCount } from "./token-format";

type ContextBreakdownSession = {
  context_prompt_tokens: number;
  context_window_tokens: number;
  context_breakdown?: {
    system_prompt_tokens: number;
    tools_and_agents_tokens: number;
    conversation_tokens: number;
    connectors_and_mcp_tokens: number;
    skills_tokens: number;
  } | null;
};

type ContextLegendSegment = {
  key: string;
  label: string;
  tokens: number;
  color: string;
  /** 占分项总和的相对份额（0~1），供环形图分扇 */
  share: number;
};

/**
 * 解析上下文分项并映射为环形图扇区与图例。
 *
 * 份额分母是分项总和而非上下文窗口——整体占用很低时，
 * 各分项在环形图上依然有可辨认的扇区。
 *
 * @param session 会话用量数据
 * @param t 中英文文案函数
 * @returns 图例分段；无分项或分项全为零时返回 null
 */
export function resolveContextBreakdown(
  session: ContextBreakdownSession,
  t: (en: string, zh: string) => string
): { segments: ContextLegendSegment[] } | null {
  const raw = session.context_breakdown;
  if (!raw) return null;
  // 1. 固定分项顺序与配色，对齐参考图例
  const items: Array<{ key: string; label: string; tokens: number; color: string }> = [
    { key: "system", label: t("System prompt", "系统提示词"), tokens: raw.system_prompt_tokens, color: "var(--context-system)" },
    { key: "tools", label: t("Tools & subagents", "工具及子智能体"), tokens: raw.tools_and_agents_tokens, color: "var(--context-tools)" },
    { key: "conversation", label: t("Conversation", "对话消息"), tokens: raw.conversation_tokens, color: "var(--context-conversation)" },
    { key: "connectors", label: t("Connectors & MCP", "连接器及MCP"), tokens: raw.connectors_and_mcp_tokens, color: "var(--context-connectors)" },
    { key: "skills", label: t("Skills", "技能"), tokens: raw.skills_tokens, color: "var(--context-skills)" }
  ];
  // 2. 按分项总和求相对份额；全为零时退回单色摘要展示
  const estimatedTotal = items.reduce((sum, item) => sum + Math.max(0, item.tokens), 0);
  if (estimatedTotal <= 0) return null;
  const segments = items.map((item) => {
    const tokens = Math.max(0, item.tokens);
    return {
      key: item.key,
      label: item.label,
      tokens,
      color: item.color,
      share: tokens / estimatedTotal
    };
  });
  return { segments };
}

/**
 * 格式化上下文占用百分比，保留一位小数。
 *
 * @param ratio 0~1 比例
 * @returns 百分比文本
 */
export function formatContextPercent(ratio: number): string {
  const value = Math.min(100, Math.max(0, ratio * 100));
  if (value > 0 && value < 0.1) return "<0.1%";
  return `${value.toFixed(1).replace(/\.0$/, "")}%`;
}

/**
 * 格式化上下文缓存命中、未命中和写入明细。
 *
 * @param cache 缓存 Token 明细
 * @param t 中英文文案函数
 * @returns 始终包含写入量的缓存明细
 */
export function formatContextCacheDetail(
  cache: { hit_tokens: number; miss_tokens: number; write_tokens: number },
  t: (en: string, zh: string) => string
): string {
  return `${formatTokenCount(cache.hit_tokens)} ${t("hit", "命中")} · ${formatTokenCount(cache.miss_tokens)} ${t("miss", "未命中")} · ${formatTokenCount(cache.write_tokens)} ${t("write", "写入")}`;
}

/**
 * 格式化图例中的约略 token 数。
 *
 * @param value token 数
 * @returns 带波浪号的紧凑文本
 */
export function formatTokenApprox(value: number): string {
  return `~${formatTokenCount(Math.max(0, value))}`;
}

/**
 * 格式化最近一次压缩原因。
 *
 * @param reason 后端 checkpoint 原因
 * @returns 中文原因标签
 */
export function formatCompactionReason(reason: "auto" | "manual" | "legacy" | null | undefined, t: (en: string, zh: string) => string): string {
  if (reason === "manual") return t("Manual", "手动");
  if (reason === "legacy") return t("Legacy migration", "旧记录迁移");
  return t("Automatic", "自动");
}

/**
 * 格式化字节数。
 *
 * @param value 字节数
 * @returns 内存大小文本
 */
export function formatBytes(value: number | null | undefined, t: (en: string, zh: string) => string): string {
  if (!value) return t("Unavailable", "不可用");
  const units = ["B", "KiB", "MiB", "GiB"];
  let amount = value;
  let index = 0;
  while (amount >= 1024 && index < units.length - 1) {
    amount /= 1024;
    index += 1;
  }
  return `${amount.toFixed(index > 1 ? 1 : 0)} ${units[index]}`;
}

/**
 * 格式化服务运行时间。
 *
 * @param seconds 运行秒数
 * @returns 运行时间文本
 */
export function formatDuration(seconds: number, locale: "en-US" | "zh-CN"): string {
  if (seconds < 60) return locale === "zh-CN" ? `运行 ${seconds} 秒` : `Up ${seconds}s`;
  if (seconds < 3_600) return locale === "zh-CN" ? `运行 ${Math.floor(seconds / 60)} 分钟` : `Up ${Math.floor(seconds / 60)}m`;
  return locale === "zh-CN"
    ? `运行 ${Math.floor(seconds / 3_600)} 小时 ${Math.floor(seconds % 3_600 / 60)} 分钟`
    : `Up ${Math.floor(seconds / 3_600)}h ${Math.floor(seconds % 3_600 / 60)}m`;
}
