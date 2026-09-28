import { ChoicePills, SettingsField, SkTextInput } from "../kit";
import type { MemorySummary } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import {
  countMemories,
  type MemoryScopeFilter,
  type MemoryTypeFilter
} from "./memory-filter";

type MemoryFilterBarProps = {
  entries: MemorySummary[];
  type: MemoryTypeFilter;
  scope: MemoryScopeFilter;
  query: string;
  onTypeChange: (type: MemoryTypeFilter) => void;
  onScopeChange: (scope: MemoryScopeFilter) => void;
  onQueryChange: (query: string) => void;
};

/** 类型 chip 的双语文案。 */
const TYPE_LABELS: Array<{ value: MemoryTypeFilter; en: string; zh: string }> = [
  { value: "all", en: "All", zh: "全部" },
  { value: "user", en: "User", zh: "用户" },
  { value: "feedback", en: "Feedback", zh: "要求" },
  { value: "project", en: "Project", zh: "项目" },
  { value: "reference", en: "Reference", zh: "资源" }
];

/** 作用域 chip 的双语文案。 */
const SCOPE_LABELS: Array<{ value: MemoryScopeFilter; en: string; zh: string }> = [
  { value: "all", en: "All", zh: "全部" },
  { value: "project", en: "Project", zh: "项目" },
  { value: "global", en: "Global", zh: "全局" }
];

/**
 * 记忆列表的筛选栏：类型、作用域与关键字。
 *
 * 角标显示每个桶的条数且只按单一维度统计，切换条件时数字不跳动。
 *
 * @param props 全部条目与当前筛选条件及回调
 * @returns 筛选栏
 */
export function MemoryFilterBar({
  entries,
  type,
  scope,
  query,
  onTypeChange,
  onScopeChange,
  onQueryChange
}: MemoryFilterBarProps) {
  const { t } = useI18n();
  const counts = countMemories(entries);

  return <div className="grid grid-cols-1 gap-3 lg:grid-cols-[minmax(0,max-content)_minmax(0,max-content)]">
    <SettingsField label={t("Search", "搜索")} anchor="memory.search" span="full"><SkTextInput type="search" value={query} onChange={onQueryChange} placeholder={t("Filter by identifier or summary", "按标识或摘要筛选")} /></SettingsField>
    <SettingsField label={t("Memory type", "记忆类型")} anchor="memory.type"><ChoicePills value={type} onChange={onTypeChange} options={TYPE_LABELS.map((option) => ({ value: option.value, label: `${t(option.en, option.zh)} ${counts.types[option.value]}` }))} /></SettingsField>
    <SettingsField label={t("Memory scope", "记忆作用域")} anchor="memory.scope"><ChoicePills value={scope} onChange={onScopeChange} options={SCOPE_LABELS.map((option) => ({ value: option.value, label: `${t(option.en, option.zh)} ${counts.scopes[option.value]}` }))} /></SettingsField>
  </div>;
}
