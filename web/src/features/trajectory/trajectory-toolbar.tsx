import { ChevronsDownUp, ChevronsUpDown, Clock, Download, ListOrdered, Search, X } from "lucide-react";
import { useState } from "react";
import { api } from "../../api/client";
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
  /** 当前会话标识，用于导出真实 HTTP 调试记录 */
  sessionId?: string;
};

/** 过滤按钮的展示顺序；与记录在轮内的出现顺序一致。 */
const KIND_ORDER: TrajectoryRecordKind[] = ["system", "user", "assistant", "tool", "subagent", "message", "compaction"];

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
  total,
  sessionId
}: TrajectoryToolbarProps) {
  const { t, locale } = useI18n();
  const [exporting, setExporting] = useState(false);
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const zh = locale.startsWith("zh");
  const durationMode = mode === "duration";

  /** 导出服务端保存的最近一次真实请求体和响应文件。 */
  const exportLatestDebug = async () => {
    if (!sessionId || exporting) return;
    setExporting(true);
    setExportStatus(null);
    try {
      const debug = await api.sessions.debugLatest(sessionId);
      if (!debug.found) {
        setExportStatus(t("No debug request has been recorded", "尚未记录调试请求"));
        return;
      }
      const blob = new Blob([JSON.stringify(debug, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `sai-${sessionId}-latest-api-debug.json`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
      setExportStatus(t("Latest real API request exported", "已导出最近一次真实 API 请求"));
    } catch (error) {
      setExportStatus(error instanceof Error ? error.message : t("Export failed", "导出失败"));
    } finally {
      setExporting(false);
    }
  };

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
        <Button
          onClick={() => void exportLatestDebug()}
          disabled={!sessionId || exporting}
          title={t("Export the latest real API request and response", "导出最近一次真实 API 请求与响应")}
        >
          <Download size={13} aria-hidden />
          {exporting ? t("Exporting", "导出中") : t("Export API", "导出 API")}
        </Button>
      </div>
      {exportStatus && <span className="trajectory-toolbar-status" role="status">{exportStatus}</span>}
    </div>
  );
}
