import { Database } from "lucide-react";
import type { TurnUsage } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { formatTurnElapsed } from "../live-run-indicator";

/**
 * 展示一轮回复的耗时、首字延迟、输入输出 token、生成速率与缓存命中占比。
 *
 * 上下行沿用 TUI 会话总览的记号：↑ 为发给模型的上行、↓ 为返回的下行。
 * 用文本箭头而非图标——此前两枚不同形制的图标视觉重量不一致，
 * 文本字形天然同字号同基线。
 *
 * @param props 本轮耗时、首字延迟和汇总用量
 * @returns 紧凑的单轮统计行；无数据时不渲染
 */
export function TurnMetrics({
  durationMs,
  ttftMs,
  usage
}: {
  durationMs?: number | null;
  ttftMs?: number | null;
  usage?: TurnUsage | null;
}) {
  const { locale, t } = useI18n();
  const hasDuration = typeof durationMs === "number" && durationMs > 0;
  const hasTtft = typeof ttftMs === "number" && ttftMs > 0;
  const tokensPerSec = usage && hasDuration
    ? formatTokensPerSec(usage.completion_tokens, durationMs)
    : null;
  if (!hasDuration && !hasTtft && !usage) return null;
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
      {hasTtft && (
        <span className="turn-metric" title={t("Time to first token", "首字延迟")}>
          {locale.startsWith("zh")
            ? `首字延迟${formatTtft(ttftMs, true)}`
            : `TTFT ${formatTtft(ttftMs, false)}`}
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
          {tokensPerSec && (
            <span className="turn-metric" title={t("Output tokens per second", "输出速率")}>
              {tokensPerSec}/s
            </span>
          )}
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

/**
 * 格式化首字延迟。
 *
 * @param ms 毫秒
 * @param zh 是否中文
 * @returns 如 `420ms` / `1.2秒`
 */
export function formatTtft(ms: number, zh: boolean): string {
  if (ms < 1_000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1_000;
  if (seconds < 10) {
    const compact = seconds.toFixed(1);
    return zh ? `${compact}秒` : `${compact}s`;
  }
  return formatTurnElapsed(ms, zh);
}

/**
 * 按生成耗时计算下行 tokens/s。
 *
 * @param completionTokens 本轮输出 token
 * @param durationMs 从首字到结束的耗时
 * @returns 有有效速率时返回紧凑数字文本
 */
export function formatTokensPerSec(completionTokens: number, durationMs: number): string | null {
  if (completionTokens <= 0 || durationMs <= 0) return null;
  const rate = completionTokens * 1_000 / durationMs;
  return rate < 10 ? rate.toFixed(1) : rate.toFixed(0);
}
