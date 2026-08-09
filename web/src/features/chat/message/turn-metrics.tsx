import { Database } from "lucide-react";
import type { TurnUsage } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { formatTurnElapsed } from "../live-run-indicator";

/**
 * 展示一轮回复的耗时、输入输出 token 与缓存命中占比。
 *
 * 上下行沿用 TUI 会话总览的记号：↑ 为发给模型的上行、↓ 为返回的下行。
 * 用文本箭头而非图标——此前两枚不同形制的图标视觉重量不一致，
 * 文本字形天然同字号同基线。
 *
 * @param props 本轮耗时和汇总用量
 * @returns 紧凑的单轮统计行；无数据时不渲染
 */
export function TurnMetrics({
  durationMs,
  usage
}: {
  durationMs?: number | null;
  usage?: TurnUsage | null;
}) {
  const { locale, t } = useI18n();
  const hasDuration = typeof durationMs === "number" && durationMs > 0;
  if (!hasDuration && !usage) return null;
  const cacheRatio = usage && usage.prompt_tokens > 0
    ? Math.min(1, Math.max(0, usage.cache_read_tokens / usage.prompt_tokens))
    : 0;

  return (
    <div className="turn-duration-meta turn-metrics" role="status">
      {hasDuration && (
        <span>
          {locale.startsWith("zh")
            ? `处理用时${formatTurnElapsed(durationMs, true)}`
            : `Processing time ${formatTurnElapsed(durationMs, false)}`}
        </span>
      )}
      {usage && (
        <>
          <span className="turn-metric" title={t("Input tokens", "输入 token")}>
            <span aria-hidden>↑</span>
            {formatTokenCount(usage.prompt_tokens)}
          </span>
          <span className="turn-metric" title={t("Output tokens", "输出 token")}>
            <span aria-hidden>↓</span>
            {formatTokenCount(usage.completion_tokens)}
          </span>
          <span className="turn-metric" title={t("Cache hit ratio for this turn", "本轮缓存命中占比")}>
            <Database size={12} aria-hidden />
            {(cacheRatio * 100).toFixed(1)}%
          </span>
        </>
      )}
    </div>
  );
}

/**
 * 格式化 token 数，保留足以比较的有效位。
 *
 * @param value token 数
 * @returns 紧凑显示文本
 */
export function formatTokenCount(value: number): string {
  if (value < 1_000) return String(Math.max(0, Math.round(value)));
  const scaled = value / 1_000;
  return `${scaled.toFixed(scaled < 10 ? 1 : 0)}k`;
}
