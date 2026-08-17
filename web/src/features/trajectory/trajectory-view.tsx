import { useMemo, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { SessionContextPrompt, SessionTimeline, SubagentDetail } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";
import { buildTrajectory } from "./trajectory-build";
import { TrajectoryDetails } from "./trajectory-details";
import { countByTurn, filterRecords } from "./trajectory-filter";
import { TrajectoryOverview } from "./trajectory-overview";
import type { TrajectoryRecordKind } from "./trajectory-record";
import { trajectoryDomain, type TimeDomain, type TrajectoryScaleMode } from "./trajectory-scale";
import { TrajectorySplit } from "./trajectory-split";
import { referencedSubagentIds } from "./trajectory-subagent";
import { TrajectoryTable } from "./trajectory-table";
import { TrajectoryToolbar } from "./trajectory-toolbar";
import "./trajectory-view.css";

type TrajectoryViewProps = {
  sessionId?: string;
  timeline: SessionTimeline | undefined;
  /** 当前会话的系统提示词快照；每次请求都会重发，作为轨迹首条记录 */
  contextPrompt?: SessionContextPrompt;
  loading: boolean;
};

/** 初始不隐藏任何种类。 */
const NO_HIDDEN_KINDS: ReadonlySet<TrajectoryRecordKind> = new Set();

/**
 * 渲染会话的调用轨迹视图。
 *
 * 时间轴、记录表与详情共用一份记录集合：三者显示的是同一批数据的
 * 三种投影，任一处的选择都能在另两处对上位置。记录表在左作总览，
 * 详情在右看完整文本，中间可拖拽调宽。
 *
 * @param props 会话时间线与加载状态
 * @returns 轨迹视图
 */
export function TrajectoryView({ sessionId, timeline, contextPrompt, loading }: TrajectoryViewProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<TrajectoryScaleMode>("duration");
  const [hiddenKinds, setHiddenKinds] = useState<ReadonlySet<TrajectoryRecordKind>>(NO_HIDDEN_KINDS);
  const [collapsedTurns, setCollapsedTurns] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [range, setRange] = useState<TimeDomain | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const debugRequests = useQuery({
    queryKey: ["session-debug-requests", sessionId],
    queryFn: () => api.sessions.debugRequests(sessionId!),
    enabled: Boolean(sessionId),
    staleTime: 15_000
  });

  // 先建一次拿到被引用的子智能体，再按 id 取详情织入——
  // 列表接口不含 timeline，只有详情接口才有分步记录
  const base = useMemo(
    () => buildTrajectory(timeline, contextPrompt, undefined, debugRequests.data),
    [timeline, contextPrompt, debugRequests.data]
  );
  const subagentIds = useMemo(() => referencedSubagentIds(base.records), [base.records]);
  const subagentQueries = useQueries({
    queries: subagentIds.map((id) => ({
      queryKey: ["subagent", id],
      queryFn: () => api.subagents.detail(id),
      staleTime: 15_000
    }))
  });
  const subagentRevision = subagentQueries
    .map((query) => `${query.data?.id ?? ""}:${query.data?.updated_at ?? 0}`)
    .join(",");
  const subagents = useMemo(() => {
    const found = new Map<string, SubagentDetail>();
    for (const query of subagentQueries) {
      if (query.data) found.set(query.data.id, query.data);
    }
    return found;
  }, [subagentRevision]);
  const model = useMemo(
    () => (subagents.size > 0
      ? buildTrajectory(timeline, contextPrompt, subagents, debugRequests.data)
      : base),
    [base, timeline, contextPrompt, subagents, debugRequests.data]
  );
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
  const selectedResultRef = selected?.detail.resultRef ?? null;
  const fullToolResult = useQuery({
    queryKey: ["trajectory-tool-result", sessionId, selectedResultRef],
    queryFn: () => api.sessions.toolResult(sessionId!, selectedResultRef!),
    enabled: Boolean(sessionId && selectedResultRef && selected?.kind === "tool"),
    staleTime: 60_000
  });
  const selectedWithFullOutput = useMemo(() => {
    if (!selected || !fullToolResult.data || selected.kind !== "tool") return selected;
    return {
      ...selected,
      detail: { ...selected.detail, output: fullToolResult.data.content }
    };
  }, [selected, fullToolResult.data]);
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
        sessionId={sessionId}
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
        <TrajectorySplit
          leftLabel={t("Overview", "总览")}
          rightLabel={t("Details", "详情")}
          left={(
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
          )}
          right={<TrajectoryDetails record={selectedWithFullOutput} onClose={() => setSelectedId(null)} />}
        />
      </div>
    </div>
  );
}
