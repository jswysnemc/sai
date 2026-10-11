import { ChoicePills, SkTextInput } from "../kit";
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
 * 记忆列表的紧凑筛选栏：关键字、类型与作用域。
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

  return (
    <div className="memory-filter-bar">
      <SkTextInput
        type="search"
        value={query}
        onChange={onQueryChange}
        placeholder={t("Filter by identifier or summary", "按标识或摘要筛选")}
        aria-label={t("Search memories", "搜索记忆")}
      />
      <ChoicePills
        value={type}
        onChange={onTypeChange}
        options={TYPE_LABELS.map((option) => ({
          value: option.value,
          label: `${t(option.en, option.zh)} ${counts.types[option.value]}`
        }))}
      />
      <ChoicePills
        value={scope}
        onChange={onScopeChange}
        options={SCOPE_LABELS.map((option) => ({
          value: option.value,
          label: `${t(option.en, option.zh)} ${counts.scopes[option.value]}`
        }))}
      />
    </div>
  );
}
