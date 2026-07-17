import type { Subagent } from "../../api/contracts";

/**
 * 从子智能体统计对象读取数值字段。
 *
 * @param stats 统计对象
 * @param key 字段名
 * @returns 数值,缺失时为 undefined
 */
function readNumber(stats: Record<string, unknown> | undefined, key: string): number | undefined {
  const value = stats?.[key];
  return typeof value === "number" ? value : undefined;
}

/**
 * 渲染子智能体运行统计:工具调用次数与 token 消耗。
 *
 * @param props 子智能体快照
 * @returns 统计视图,无统计时返回 null
 */
export function SubagentStats({ subagent }: { subagent: Subagent }) {
  const stats = subagent.stats;
  if (!stats) return null;
  const toolCalls = readNumber(stats, "tool_calls");
  const tokens = readNumber(stats, "token_estimate");
  const isActual = stats["token_estimate_is_actual"] === true;
  const items: Array<{ label: string; value: string }> = [];
  if (toolCalls !== undefined) items.push({ label: "工具调用", value: `${toolCalls} 次` });
  if (tokens !== undefined) items.push({ label: "Token", value: `${isActual ? "" : "~"}${formatCount(tokens)}` });
  if (items.length === 0) return null;
  return (
    <dl className="subagent-stats">
      {items.map((item) => (
        <div key={item.label}><dt>{item.label}</dt><dd>{item.value}</dd></div>
      ))}
    </dl>
  );
}

/**
 * 将数量格式化为易读文本。
 *
 * @param value 原始数量
 * @returns 带 K/M 单位的文本
 */
function formatCount(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
}
