import { Archive, SlidersHorizontal } from "lucide-react";
import { localizeApiMessage } from "../../api/api-error";
import type { SystemUsage } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { ContextDonut } from "./context-donut";
import { formatTokenCount } from "./token-format";
import { formatCompactionReason, formatContextCacheDetail, formatContextPercent, formatTokenApprox, resolveContextBreakdown } from "./usage-format";

/**
 * 紧凑展示上下文构成、缓存与压缩操作。
 * @param props 会话用量、压缩状态及操作回调
 * @returns 上下文详情内容
 */
export function ContextUsagePanel({ usage, compactPending, compactDisabled, compactError, onCompact, onConfigure }: {
  usage: SystemUsage;
  compactPending: boolean;
  compactDisabled: boolean;
  compactError?: string;
  onCompact: () => void;
  onConfigure: () => void;
}) {
  const { locale, t } = useI18n();
  const { session } = usage;
  const breakdown = resolveContextBreakdown(session, t);
  return (
    <section className="context-usage-card">
      <div className="context-usage-chart">
        <ContextDonut
          segments={breakdown?.segments.map((segment) => ({ ...segment, title: `${segment.label} ${formatTokenApprox(segment.tokens)}` })) ?? []}
          percentLabel={formatContextPercent(session.context_token_ratio)}
          usedLabel={`${formatTokenCount(session.context_prompt_tokens)}/${formatTokenCount(session.context_window_tokens)}`}
          ariaLabel={t("Context usage breakdown", "上下文用量构成")}
        />
        {breakdown ? <ul className="context-usage-legend">
          {breakdown.segments.map((segment) => <li key={segment.key}>
            <i style={{ background: segment.color }} /><span title={segment.label}>{segment.label}</span><strong>{formatTokenApprox(segment.tokens)}</strong>
          </li>)}
        </ul> : <p className="usage-loading">{t("Context breakdown unavailable", "暂无上下文构成数据")}</p>}
      </div>
      {session.context_cache && <div className="context-cache-summary">
        <span>{t("Cache hit", "缓存命中")} <strong>{formatContextPercent(session.context_cache.hit_ratio)}</strong></span>
        <small>{formatContextCacheDetail(session.context_cache, t)}</small>
      </div>}
      <dl className="usage-session-totals">
        <div><dt>{t("Input / output", "输入 / 输出")}</dt><dd>{formatTokenCount(session.prompt_tokens)} / {formatTokenCount(session.completion_tokens)}</dd></div>
        <div><dt>{t("Requests / tools", "请求 / 工具")}</dt><dd>{session.requests} / {session.tool_calls}</dd></div>
      </dl>
      <div className="context-compaction-status">{session.checkpoint_count > 0
        ? t(`Compacted ${session.compacted_turns} turns · ${formatCompactionReason(session.latest_checkpoint_reason, t)}`, `已压缩 ${session.compacted_turns} 轮 · ${formatCompactionReason(session.latest_checkpoint_reason, t)}`)
        : t("Not compacted", "尚未压缩")}</div>
      <div className="context-compaction-actions">
        <Button variant="ghost" size="small" onClick={onConfigure}><SlidersHorizontal size={13} />{t("Settings", "压缩设置")}</Button>
        <Button size="small" onClick={onCompact} disabled={compactPending || compactDisabled || usage.runtime.active_run}><Archive size={13} />{compactPending ? t("Compacting", "正在压缩") : t("Compact now", "手动压缩")}</Button>
      </div>
      {compactError && <p className="usage-error" role="alert">{compactError}</p>}
      {session.compaction_warning && <p className="context-compaction-result">{localizeApiMessage(session.compaction_warning, locale)}</p>}
    </section>
  );
}
