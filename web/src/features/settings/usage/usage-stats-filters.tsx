import type { UsageRange } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
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
  return (
    <div className="usage-filters">
      <div className="settings-field">
        <span>{t("Range", "时间范围")}</span>
        <Select
          value={value.range}
          options={RANGES.map((item) => ({ value: item, label: rangeLabel(item, t) }))}
          ariaLabel={t("Range", "时间范围")}
          onChange={(next) => onChange({ range: next as UsageRange })}
        />
      </div>
      <div className="settings-field">
        <span>{t("Source", "来源")}</span>
        <Select
          value={value.source}
          options={SOURCES.map((item) => ({ value: item, label: sourceLabel(item, t) }))}
          ariaLabel={t("Source", "来源")}
          onChange={(next) => onChange({ source: next })}
        />
      </div>
      <div className="settings-field">
        <span>{t("Status", "状态")}</span>
        <Select
          value={value.status}
          options={STATUSES.map((item) => ({ value: item, label: statusLabel(item, t) }))}
          ariaLabel={t("Status", "状态")}
          onChange={(next) => onChange({ status: next })}
        />
      </div>
      <label className="settings-field">
        <span>{t("Provider", "供应商")}</span>
        <input
          value={value.providerSearch}
          onChange={(event) => onChange({ providerSearch: event.target.value })}
          placeholder={t("Search provider", "搜索供应商")}
        />
      </label>
      <label className="settings-field">
        <span>{t("Model", "模型")}</span>
        <input
          value={value.modelSearch}
          onChange={(event) => onChange({ modelSearch: event.target.value })}
          placeholder={t("Search model", "搜索模型")}
        />
      </label>
    </div>
  );
}
