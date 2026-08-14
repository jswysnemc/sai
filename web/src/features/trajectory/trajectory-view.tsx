import { useMemo, useState } from "react";
import type { SessionTimeline } from "../../api/contracts";
import { buildTrajectory } from "./trajectory-build";
import { TrajectoryDetails } from "./trajectory-details";
import { countByTurn, filterRecords } from "./trajectory-filter";
import { TrajectoryOverview } from "./trajectory-overview";
import type { TrajectoryRecordKind } from "./trajectory-record";
import { trajectoryDomain, type TimeDomain, type TrajectoryScaleMode } from "./trajectory-scale";
import { TrajectoryTable } from "./trajectory-table";
import { TrajectoryToolbar } from "./trajectory-toolbar";
import "./trajectory-view.css";

type TrajectoryViewProps = {
  timeline: SessionTimeline | undefined;
  loading: boolean;
};

/** 初始不隐藏任何种类。 */
const NO_HIDDEN_KINDS: ReadonlySet<TrajectoryRecordKind> = new Set();

/**
 * 渲染会话的调用轨迹视图。
 *
 * 概览、记录表与详情共用一份记录集合：三者显示的是同一批数据的
 * 三种投影，任一处的选择都能在另两处对上位置。
 *
 * @param props 会话时间线与加载状态
 * @returns 轨迹视图
 */
export function TrajectoryView({ timeline, loading }: TrajectoryViewProps) {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<TrajectoryScaleMode>("duration");
  const [hiddenKinds, setHiddenKinds] = useState<ReadonlySet<TrajectoryRecordKind>>(NO_HIDDEN_KINDS);
  const [collapsedTurns, setCollapsedTurns] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [range, setRange] = useState<TimeDomain | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const model = useMemo(() => buildTrajectory(timeline), [timeline]);
  const bounds = useMemo(() => trajectoryDomain(model.records), [model.records]);
  const turnCounts = useMemo(() => countByTurn(model.records), [model.records]);
  const visible = useMemo(
    () => filterRecords(model.records, { query, hiddenKinds, range, collapsedTurns }),
    [model.records, query, hiddenKinds, range, collapsedTurns]
  );
  const selected = useMemo(
    () => model.records.find((record) => record.id === selectedId) ?? null,
    [model.records, selectedId]
  );
  const collapsibleTurnIds = useMemo(
    () => model.turns.filter((turn) => (turnCounts.get(turn.turnId) ?? 0) > 1).map((turn) => turn.turnId),
    [model.turns, turnCounts]
  );
  const allCollapsed = collapsibleTurnIds.length > 0
    && collapsibleTurnIds.every((turnId) => collapsedTurns.has(turnId));

  /**
   * 切换某个记录种类的可见性。
   *
   * @param kind 记录种类
   * @returns 无
   */
  const toggleKind = (kind: TrajectoryRecordKind) => {
    setHiddenKinds((current) => {
      const next = new Set(current);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      return next;
    });
  };

  /**
   * 切换某个轮次的折叠状态。
   *
   * @param turnId 轮次标识
   * @returns 无
   */
  const toggleTurn = (turnId: string) => {
    setCollapsedTurns((current) => {
      const next = new Set(current);
      if (next.has(turnId)) next.delete(turnId);
      else next.add(turnId);
      return next;
    });
  };

  /**
   * 全部折叠或全部展开。
   *
   * @returns 无
   */
  const toggleAll = () => {
    setCollapsedTurns(allCollapsed ? new Set<string>() : new Set(collapsibleTurnIds));
  };

  return (
    <div className="trajectory-view">
      <TrajectoryToolbar
        query={query}
        onQueryChange={setQuery}
        mode={mode}
        onModeChange={(next) => { setMode(next); setRange(null); }}
        hiddenKinds={hiddenKinds}
        onToggleKind={toggleKind}
        allCollapsed={allCollapsed}
        onToggleAll={toggleAll}
        shown={visible.length}
        total={model.records.length}
      />
      <TrajectoryOverview
        records={model.records}
        mode={mode}
        bounds={bounds}
        range={range}
        onRangeChange={setRange}
        selectedId={selectedId}
        onSelect={setSelectedId}
      />
      <div className="trajectory-view-body" data-inspecting={selected !== null || undefined}>
        <div className="trajectory-view-ledger">
          <TrajectoryTable
            records={visible}
            turns={model.turns}
            turnCounts={turnCounts}
            collapsedTurns={collapsedTurns}
            onToggleTurn={toggleTurn}
            selectedId={selectedId}
            onSelect={setSelectedId}
            loading={loading}
          />
        </div>
        <TrajectoryDetails record={selected} onClose={() => setSelectedId(null)} />
      </div>
    </div>
  );
}
