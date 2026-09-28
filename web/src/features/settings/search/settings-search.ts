import type { Locale } from "../../i18n/locale";
import { getSettingsSection } from "../settings-registry";
import { SETTINGS_SEARCH_INDEX } from "./settings-search-index";
import type { SettingsSearchEntry } from "./settings-search-types";

/** 字段搜索结果：条目、得分与展示用的位置路径。 */
export type SettingsSearchHit = {
  entry: SettingsSearchEntry;
  score: number;
  /** 当前语言下的字段名称 */
  label: string;
  /** 当前语言下的位置，例如「运行时 › 工具与上下文」 */
  location: string;
};

/**
 * 归一化搜索文本：小写并去掉空白、下划线、连字符与点号，
 * 使「api key」「api_key」「apikey」互相命中。
 *
 * @param value 原始文本
 * @returns 归一化文本
 */
export function normalizeSearchText(value: string): string {
  return value.toLowerCase().replace(/[\s_.\-]+/g, "");
}

/**
 * 计算单个条目与关键词的匹配得分。
 *
 * @param entry 字段条目
 * @param needle 归一化后的关键词
 * @returns 得分；0 表示不匹配
 */
export function scoreSearchEntry(entry: SettingsSearchEntry, needle: string): number {
  if (!needle) return 0;
  const labels = [entry.labelZh, entry.labelEn].map(normalizeSearchText);
  // 1. 标签完全相同得分最高，其次是前缀，再次是包含
  if (labels.some((label) => label === needle)) return 100;
  if (labels.some((label) => label.startsWith(needle))) return 80;
  if (labels.some((label) => label.includes(needle))) return 60;
  // 2. 关键词（配置键、同义词）命中得分较低
  const keywords = (entry.keywords ?? []).map(normalizeSearchText);
  if (keywords.some((keyword) => keyword === needle)) return 50;
  if (keywords.some((keyword) => keyword.includes(needle))) return 30;
  return 0;
}

/**
 * 按关键词搜索设置字段。
 *
 * @param query 用户输入
 * @param locale 当前界面语言，决定展示文字
 * @param entries 字段索引，默认使用全局索引
 * @param limit 最多返回条数
 * @returns 按得分降序的结果
 */
export function searchSettingsFields(
  query: string,
  locale: Locale,
  entries: readonly SettingsSearchEntry[] = SETTINGS_SEARCH_INDEX,
  limit = 12
): SettingsSearchHit[] {
  const needle = normalizeSearchText(query.trim());
  if (!needle) return [];
  const zh = locale === "zh-CN";
  return entries
    .map((entry) => ({ entry, score: scoreSearchEntry(entry, needle) }))
    .filter((hit) => hit.score > 0)
    .sort((left, right) => right.score - left.score)
    .slice(0, limit)
    .map(({ entry, score }) => ({
      entry,
      score,
      label: zh ? entry.labelZh : entry.labelEn,
      location: searchEntryLocation(entry, zh)
    }));
}

/**
 * 生成条目所在位置的展示文字。
 *
 * @param entry 字段条目
 * @param zh 是否使用中文
 * @returns 例如「运行时 › 工具与上下文」
 */
function searchEntryLocation(entry: SettingsSearchEntry, zh: boolean): string {
  const section = getSettingsSection(entry.section);
  if (!section) return entry.section;
  const parts = [zh ? section.labelZh : section.labelEn];
  const subview = section.subviews?.find((item) => item.id === entry.subview);
  if (subview) parts.push(zh ? subview.labelZh : subview.labelEn);
  return parts.join(" › ");
}

/**
 * 生成跳转到字段的设置页地址。
 *
 * @param entry 字段条目
 * @returns 带 focus 参数的地址
 */
export function searchEntryHref(entry: SettingsSearchEntry): string {
  const path = entry.subview ? `/settings/${entry.section}/${entry.subview}` : `/settings/${entry.section}`;
  return `${path}?focus=${encodeURIComponent(entry.anchor)}${entry.item ? `&item=${encodeURIComponent(entry.item)}` : ""}`;
}
