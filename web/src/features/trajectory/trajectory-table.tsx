import { Fragment, useMemo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";
import { formatDuration } from "./trajectory-format";
import type { TrajectoryTurnHeader } from "./trajectory-build";
import type { TrajectoryRecord } from "./trajectory-record";
import { TrajectoryRow } from "./trajectory-row";
import "./trajectory-table.css";

type TrajectoryTableProps = {
  records: readonly TrajectoryRecord[];
  turns: readonly TrajectoryTurnHeader[];
  /** 每轮的记录总数，用于折叠行的计数 */
  turnCounts: ReadonlyMap<string, number>;
  collapsedTurns: ReadonlySet<string>;
  onToggleTurn: (turnId: string) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
  loading: boolean;
};

/**
 * 渲染按轮次分组的轨迹记录表。
 *
 * @param props 记录、轮次分隔数据与选中、折叠状态
 * @returns 记录表
 */
export function TrajectoryTable({
  records,
  turns,
  turnCounts,
  collapsedTurns,
  onToggleTurn,
  selectedId,
  onSelect,
  loading
}: TrajectoryTableProps) {
  const { t } = useI18n();
  const headers = useMemo(
    () => new Map(turns.map((turn) => [turn.turnId, turn])),
    [turns]
  );

  if (loading) {
    return <div className="trajectory-table-empty">{t("Loading trajectory", "正在读取轨迹")}</div>;
  }
  if (records.length === 0) {
    return <div className="trajectory-table-empty">{t("No records match the current filters", "没有符合筛选条件的记录")}</div>;
  }

  return (
    <div className="trajectory-table" role="table" aria-label={t("Trajectory records", "轨迹记录")}>
      {records.map((record) => {
        const header = record.turnStart && record.turnId ? headers.get(record.turnId) : undefined;
        const collapsed = record.turnId ? collapsedTurns.has(record.turnId) : false;
        const hidden = collapsed && record.turnId
          ? Math.max(0, (turnCounts.get(record.turnId) ?? 1) - 1)
          : 0;
        return (
          <Fragment key={record.id}>
            {header && (
              <TurnDivider
                header={header}
                collapsed={collapsed}
                onToggle={() => onToggleTurn(header.turnId)}
              />
            )}
            <TrajectoryRow
              record={record}
              selected={record.id === selectedId}
              collapsedCount={hidden}
              onSelect={onSelect}
              onToggleTurn={onToggleTurn}
            />
          </Fragment>
        );
      })}
    </div>
  );
}

/**
 * 渲染一条轮次分隔行。
 *
 * @param props 轮次数据与折叠状态
 * @returns 轮次分隔行
 */
function TurnDivider({
  header,
  collapsed,
  onToggle
}: {
  header: TrajectoryTurnHeader;
  collapsed: boolean;
  onToggle: () => void;
}) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      className="trajectory-turn-divider"
      data-status={header.status}
      aria-expanded={!collapsed}
      onClick={onToggle}
    >
      {collapsed ? <ChevronRight size={12} aria-hidden /> : <ChevronDown size={12} aria-hidden />}
      <strong>{t(`Turn ${header.seq}`, `第 ${header.seq} 轮`)}</strong>
      <span className="trajectory-turn-requests">
        {t(`${header.requestCount} requests`, `${header.requestCount} 次请求`)}
      </span>
      {header.model && <code className="trajectory-turn-model">{header.model}</code>}
      <span className="trajectory-turn-duration">{formatDuration(header.durationMs)}</span>
    </button>
  );
}
