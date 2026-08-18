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
      {records.map((record, index) => {
        const previous = records[index - 1];
        const header = record.turnId && record.turnId !== previous?.turnId
          ? headers.get(record.turnId)
          : undefined;
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
            {record.kind === "compaction" ? (
              <CompactionDivider
                record={record}
                selected={record.id === selectedId}
                onSelect={onSelect}
              />
            ) : (
              <TrajectoryRow
                record={record}
                selected={record.id === selectedId}
                collapsedCount={hidden}
                onSelect={onSelect}
                onToggleTurn={onToggleTurn}
              />
            )}
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

/**
 * 渲染压缩摘要的分界行。
 *
 * 这是已清出窗口的旧轮次与后面仍保留轮次之间的边界，
 * 点开后在详情栏读全文，不在表里把摘要撑成一块卡片。
 *
 * @param props 压缩记录与选中状态
 * @returns 压缩分界行
 */
function CompactionDivider({
  record,
  selected,
  onSelect
}: {
  record: TrajectoryRecord;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  const { t } = useI18n();
  const from = record.detail.compactedFromSeq ?? 0;
  const to = record.detail.compactedToSeq ?? 0;
  const range = from > 0 && to > 0
    ? from === to
      ? t(`Turn ${from} compacted`, `第 ${from} 轮已压缩`)
      : t(`Turns ${from}–${to} compacted`, `第 ${from}–${to} 轮已压缩`)
    : t("Compacted context", "已压缩的上下文");
  return (
    <button
      type="button"
      className="trajectory-compaction-divider"
      data-kind="compaction"
      data-selected={selected || undefined}
      aria-pressed={selected}
      onClick={() => onSelect(record.id)}
    >
      <strong>{range}</strong>
      {record.label && <span className="trajectory-turn-requests">{record.label}</span>}
      <span className="trajectory-compaction-summary">{record.summary}</span>
    </button>
  );
}
