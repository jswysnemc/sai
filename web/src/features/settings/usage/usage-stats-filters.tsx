import type { UsageRange } from "../../../api/contracts";
import { SkSelect, SkTextInput } from "../kit";
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
 * 渲染用量统计的紧凑筛选条。
 *
 * 标签与控件横排且不换行，避免窄列把中文竖排拆开。
 *
 * @param props 当前筛选值、变更回调与双语取值函数
 * @returns 筛选表单
 */
export function UsageStatsFilters({ value, onChange, t }: UsageStatsFiltersProps) {
  return (
    <div className="usage-filter-bar">
      <label className="usage-filter-item">
        <span>{t("Range", "时间范围")}</span>
        <SkSelect value={value.range} options={RANGES.map((item) => ({ value: item, label: rangeLabel(item, t) }))} onChange={(range) => onChange({ range })} />
      </label>
      <label className="usage-filter-item">
        <span>{t("Source", "来源")}</span>
        <SkSelect value={value.source} options={SOURCES.map((item) => ({ value: item, label: sourceLabel(item, t) }))} onChange={(source) => onChange({ source })} />
      </label>
      <label className="usage-filter-item">
        <span>{t("Status", "状态")}</span>
        <SkSelect value={value.status} options={STATUSES.map((item) => ({ value: item, label: statusLabel(item, t) }))} onChange={(status) => onChange({ status })} />
      </label>
      <label className="usage-filter-item is-grow">
        <span>{t("Provider", "供应商")}</span>
        <SkTextInput value={value.providerSearch} onChange={(providerSearch) => onChange({ providerSearch })} placeholder={t("Search provider", "搜索供应商")} />
      </label>
      <label className="usage-filter-item is-grow">
        <span>{t("Model", "模型")}</span>
        <SkTextInput value={value.modelSearch} onChange={(modelSearch) => onChange({ modelSearch })} placeholder={t("Search model", "搜索模型")} />
      </label>
    </div>
  );
}
