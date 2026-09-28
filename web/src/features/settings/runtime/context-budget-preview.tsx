import { useI18n } from "../../i18n/use-i18n";

/**
 * 【上下文】【预算估算】按照后端 CompactionBudgetPolicy 计算配置的压缩触发点。
 * @param limit 默认窗口大小
 * @param ratio 压缩比例
 * @param reserve 预留数量；零或超过窗口时只使用比例
 * @returns 压缩触发数量
 */
export function contextCompactionThreshold(limit: number, ratio: number, reserve: number): number {
  if (limit <= 0) return 0;
  const clamped = Math.min(0.99, Math.max(0.5, Number.isFinite(ratio) ? ratio : 0.9));
  // 1. 后端使用 f32 乘法后取整数，保留相同的边界规则
  const proportional = Math.max(1, Math.trunc(Math.fround(Math.fround(limit) * Math.fround(clamped))));
  return reserve <= 0 || reserve >= limit ? proportional : Math.max(proportional, limit - reserve);
}

/**
 * 【上下文】【预算预览】展示默认窗口的压缩水位，不代表当前会话占用。
 * @param props 默认窗口、比例和预留配置
 * @returns 紧凑预算条及可读数值
 */
export function ContextBudgetPreview({ limit, ratio, reserve }: { limit: number; ratio: number; reserve: number }) {
  const { t } = useI18n();
  const threshold = contextCompactionThreshold(limit, ratio, reserve);
  const percent = limit > 0 ? threshold / limit * 100 : 0;
  return <figure className="m-0 mt-3 grid gap-2 text-xs" aria-label={t("Default context budget preview", "默认上下文预算预览")}>
    <figcaption className="flex flex-wrap justify-between gap-2">
      <span>{t("Compaction threshold", "压缩触发点")} <strong>{threshold.toLocaleString()} tokens · {Number(percent.toFixed(1))}%</strong></span>
      <span>{t("Remaining at threshold", "触发时剩余")} {(limit - threshold).toLocaleString()} tokens</span>
    </figcaption>
    <div className="flex h-2 overflow-hidden rounded-full bg-[var(--paper-deep)]" aria-hidden="true">
      <span className="h-full bg-[var(--signal)]" style={{ width: `${percent}%` }} />
    </div>
    <p className="m-0 text-muted">{t("Estimate for the configured default window. A model-specific window takes precedence; compaction uses whichever threshold is reached later: the ratio or the remaining reserve.", "按默认窗口估算；模型专属窗口优先。比例与剩余预留量取较晚达到的条件，不表示当前会话用量。")}</p>
  </figure>;
}
