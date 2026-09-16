import type { SessionTimelineTurn } from "../../api/contracts";
import type { Locale } from "../i18n/locale";
import type { MessageOverviewItem } from "./message-overview-utils";

const historyCache = new WeakMap<SessionTimelineTurn, Partial<Record<Locale, MessageOverviewItem>>>();

/**
 * 【前端性能】【历史概览】按不可变轮次快照与语言复用概览，旧快照释放后缓存随之回收。
 * @param turn 当前历史轮次快照
 * @param locale 展示语言
 * @param create 缓存缺失时计算概览的方法
 * @returns 与轮次内容和语言对应的稳定概览对象
 */
export function cachedHistoryOverview(
  turn: SessionTimelineTurn,
  locale: Locale,
  create: (turn: SessionTimelineTurn, locale: Locale) => MessageOverviewItem
): MessageOverviewItem {
  const translations = historyCache.get(turn);
  const existing = translations?.[locale];
  if (existing) return existing;
  const item = create(turn, locale);
  historyCache.set(turn, { ...translations, [locale]: item });
  return item;
}
