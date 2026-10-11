import type { UsageDailyActivity } from "../../../api/contracts/usage";
import { formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";
import { buildRecentDays, formatDateLabel, formatShortDate } from "./usage-activity";

type Props = {
  days: UsageDailyActivity[];
  locale: "en-US" | "zh-CN";
  t: Translate;
};

/**
 * 【用量】【近况】最近 14 天 Token 柱，空日高度为 0，今天保留最小高度以免消失。
 * @param props 连续日历、语言与双语函数
 * @returns 近况柱图；窗口不足时不渲染
 */
export function UsageRecentBars({ days, locale, t }: Props) {
  const recent = buildRecentDays(days);
  const first = recent[0];
  const last = recent[recent.length - 1];
  if (!first || !last) return null;
  const peak = Math.max(0, ...recent.map((day) => day.tokens));

  return (
    <section className="usage-recent-bars" aria-label={t("Recent tokens", "近期 Token")}>
      <header>
        <span>{t("Recent tokens", "近期 Token")}</span>
        <time>{formatShortDate(first.date, locale)} – {formatShortDate(last.date, locale)}</time>
      </header>
      <ol>
        {recent.map((day, index) => {
          const isLast = index === recent.length - 1;
          const isPeak = day.tokens > 0 && day.tokens === peak;
          const height = peak > 0 ? (day.tokens / peak) * 100 : 0;
          const dateLabel = formatDateLabel(day.date, locale);
          return (
            <li key={day.date} title={`${dateLabel} · ${formatTokens(day.tokens)} ${t("tokens", "Token")}`}>
              <span
                aria-hidden="true"
                className={isLast ? "is-today" : undefined}
                style={{ height: `${height}%`, minHeight: isLast ? 2 : undefined }}
              />
              {day.tokens > 0 && (
                <em className={isLast || isPeak ? "is-visible" : undefined} style={{ bottom: `calc(${height}% + 4px)` }}>
                  {formatTokens(day.tokens)}
                </em>
              )}
              <span className="usage-recent-sr">{`${dateLabel}: ${formatTokens(day.tokens)} ${t("tokens", "Token")}`}</span>
            </li>
          );
        })}
      </ol>
      <div aria-hidden="true" className="usage-recent-axis">
        {recent.map((day, index) => (
          <span key={day.date}>{(recent.length - 1 - index) % 3 === 0 ? formatShortDate(day.date, locale) : ""}</span>
        ))}
      </div>
    </section>
  );
}
