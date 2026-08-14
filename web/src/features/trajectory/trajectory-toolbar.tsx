import { ChevronsDownUp, ChevronsUpDown, Clock, ListOrdered, Search, X } from "lucide-react";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { RECORD_KIND_LABELS, type TrajectoryRecordKind } from "./trajectory-record";
import type { TrajectoryScaleMode } from "./trajectory-scale";
import "./trajectory-toolbar.css";

type TrajectoryToolbarProps = {
  query: string;
  onQueryChange: (query: string) => void;
  mode: TrajectoryScaleMode;
  onModeChange: (mode: TrajectoryScaleMode) => void;
  hiddenKinds: ReadonlySet<TrajectoryRecordKind>;
  onToggleKind: (kind: TrajectoryRecordKind) => void;
  allCollapsed: boolean;
  onToggleAll: () => void;
  /** 当前筛选后的记录数与总数 */
  shown: number;
  total: number;
};

/** 过滤按钮的展示顺序；与记录在轮内的出现顺序一致。 */
const KIND_ORDER: TrajectoryRecordKind[] = ["user", "assistant", "tool", "message", "compaction"];

/**
 * 渲染轨迹视图的顶部工具栏。
 *
 * @param props 搜索词、横轴口径、种类过滤与折叠状态
 * @returns 工具栏
 */
export function TrajectoryToolbar({
  query,
  onQueryChange,
  mode,
  onModeChange,
  hiddenKinds,
  onToggleKind,
  allCollapsed,
  onToggleAll,
  shown,
  total
}: TrajectoryToolbarProps) {
  const { t, locale } = useI18n();
  const zh = locale.startsWith("zh");
  const durationMode = mode === "duration";

  return (
    <div className="trajectory-toolbar" role="toolbar" aria-label={t("Trajectory toolbar", "轨迹工具栏")}>
      <div className="trajectory-toolbar-search">
        <Search size={13} aria-hidden />
        <input
          type="search"
          value={query}
          placeholder={t("Search records", "搜索记录")}
          aria-label={t("Search trajectory records", "搜索轨迹记录")}
          onChange={(event) => onQueryChange(event.currentTarget.value)}
        />
        {query && (
          <button
            type="button"
            className="trajectory-toolbar-clear"
            aria-label={t("Clear search", "清除搜索")}
            onClick={() => onQueryChange("")}
          >
            <X size={12} aria-hidden />
          </button>
        )}
      </div>
      <div className="trajectory-toolbar-kinds" role="group" aria-label={t("Filter record kinds", "筛选记录种类")}>
        {KIND_ORDER.map((kind) => {
          const hidden = hiddenKinds.has(kind);
          const label = zh ? RECORD_KIND_LABELS[kind].zh : RECORD_KIND_LABELS[kind].en;
          return (
            <button
              type="button"
              key={kind}
              className="trajectory-kind-filter"
              data-kind={kind}
              data-off={hidden || undefined}
              aria-pressed={!hidden}
              title={hidden ? t(`Show ${label}`, `显示${label}`) : t(`Hide ${label}`, `隐藏${label}`)}
              onClick={() => onToggleKind(kind)}
            >
              {label}
            </button>
          );
        })}
      </div>
      <div className="trajectory-toolbar-actions">
        <span className="trajectory-toolbar-count">
          {shown === total ? t(`${total} records`, `${total} 条`) : t(`${shown} / ${total}`, `${shown} / ${total}`)}
        </span>
        <Button
          onClick={() => onModeChange(durationMode ? "sequence" : "duration")}
          aria-pressed={durationMode}
          title={durationMode
            ? t("Switch the overview to equal-width records", "概览切换为等宽排列")
            : t("Switch the overview to recorded durations", "概览切换为真实耗时")}
        >
          {durationMode ? <Clock size={13} aria-hidden /> : <ListOrdered size={13} aria-hidden />}
          {durationMode ? t("Duration", "耗时") : t("Sequence", "顺序")}
        </Button>
        <Button
          onClick={onToggleAll}
          title={allCollapsed ? t("Expand all turns", "展开所有轮次") : t("Collapse all turns", "折叠所有轮次")}
        >
          {allCollapsed ? <ChevronsUpDown size={13} aria-hidden /> : <ChevronsDownUp size={13} aria-hidden />}
          {allCollapsed ? t("Expand", "展开") : t("Collapse", "折叠")}
        </Button>
      </div>
    </div>
  );
}
