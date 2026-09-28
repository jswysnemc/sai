import type { UsageRange } from "../../../api/contracts";
import { SettingsField, SkSelect, SkTextInput } from "../kit";
import { rangeLabel, sourceLabel, statusLabel, type Translate } from "./usage-labels";

const RANGES: UsageRange[] = ["today", "1d", "7d", "30d", "90d", "all"];
const SOURCES = ["all", "chat", "compaction", "session_memory"];
const STATUSES = ["all", "success", "error", "missing_usage"];

/** 用量面板的筛选条件。 */
export type UsageFilterState = {
  range: UsageRange;
  source: string;
  status: string;
  providerSearch: string;
  modelSearch: string;
};

type UsageStatsFiltersProps = {
  value: UsageFilterState;
  onChange: (next: Partial<UsageFilterState>) => void;
  t: Translate;
};

/**
 * 渲染用量统计的筛选条。
 *
 * @param props 当前筛选值、变更回调与双语取值函数
 * @returns 筛选表单
 */
export function UsageStatsFilters({ value, onChange, t }: UsageStatsFiltersProps) {
  return <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-5">
    <SettingsField label={t("Range", "时间范围")} anchor="usage.range"><SkSelect value={value.range} options={RANGES.map((item) => ({ value: item, label: rangeLabel(item, t) }))} onChange={(range) => onChange({ range })} /></SettingsField>
    <SettingsField label={t("Source", "来源")} anchor="usage.source"><SkSelect value={value.source} options={SOURCES.map((item) => ({ value: item, label: sourceLabel(item, t) }))} onChange={(source) => onChange({ source })} /></SettingsField>
    <SettingsField label={t("Status", "状态")} anchor="usage.status"><SkSelect value={value.status} options={STATUSES.map((item) => ({ value: item, label: statusLabel(item, t) }))} onChange={(status) => onChange({ status })} /></SettingsField>
    <SettingsField label={t("Provider", "供应商")} anchor="usage.provider"><SkTextInput value={value.providerSearch} onChange={(providerSearch) => onChange({ providerSearch })} placeholder={t("Search provider", "搜索供应商")} /></SettingsField>
    <SettingsField label={t("Model", "模型")} anchor="usage.model"><SkTextInput value={value.modelSearch} onChange={(modelSearch) => onChange({ modelSearch })} placeholder={t("Search model", "搜索模型")} /></SettingsField>
  </div>;
}
