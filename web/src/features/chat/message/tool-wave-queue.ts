import { useEffect, useRef, useState } from "react";
import { useReducedMotion } from "../tool-renderers/layout/use-reduced-motion";
import type { ToolLifecycle } from "../run-event-reducer";

export type ToolWaveStatus = ToolLifecycle["status"];

export type ToolWaveEvent = {
  id: string;
  kind: "start" | "end";
};

export type ToolWaveSnapshot = {
  id: string;
  status: ToolWaveStatus;
};

/** 队列里每一帧工具行至少停留的时长，避免竞态把开始/结束一闪而过 */
export const WAVE_SLIDE_HOLD_MS = 800;

/**
 * 判断工具是否仍在调用过程中。
 *
 * @param status 工具生命周期状态
 * @returns 准备中或执行中时为 true
 */
export function isToolWaveActive(status: ToolWaveStatus): boolean {
  return status === "preparing" || status === "running";
}

/**
 * 把相邻两帧工具状态差成开始/结束事件，保持列表顺序。
 *
 * @param prev 上一帧 id → 状态
 * @param next 当前工具快照
 * @returns 应按序入队的轮播事件
 */
export function diffToolWaveEvents(
  prev: Readonly<Record<string, ToolWaveStatus>>,
  next: readonly ToolWaveSnapshot[]
): ToolWaveEvent[] {
  const events: ToolWaveEvent[] = [];
  for (const tool of next) {
    const before = prev[tool.id];
    if (before === undefined) {
      if (isToolWaveActive(tool.status)) events.push({ id: tool.id, kind: "start" });
      continue;
    }
    if (!isToolWaveActive(before) && isToolWaveActive(tool.status)) {
      events.push({ id: tool.id, kind: "start" });
    }
    if (isToolWaveActive(before) && !isToolWaveActive(tool.status)) {
      events.push({ id: tool.id, kind: "end" });
    }
  }
  return events;
}

/**
 * 把新的开始/结束事件接到轮播队列末尾。
 *
 * 连续重复的同一事件来自流式补丁，丢弃以免同一帧播两次。
 * 同一工具的 start 与 end 都保留，因为调用开始和结束要各轮播一次。
 *
 * @param queue 当前待播队列
 * @param incoming 本帧新事件
 * @returns 合并后的队列
 */
export function enqueueToolWaveEvents(
  queue: readonly ToolWaveEvent[],
  incoming: readonly ToolWaveEvent[]
): ToolWaveEvent[] {
  const result = [...queue];
  for (const event of incoming) {
    const last = result[result.length - 1];
    if (last && last.id === event.id && last.kind === event.kind) continue;
    result.push(event);
  }
  return result;
}

/**
 * 生成 id → 状态快照，供下一帧 diff。
 *
 * @param tools 当前工具快照
 * @returns 状态表
 */
export function snapshotToolWaveStatus(
  tools: readonly ToolWaveSnapshot[]
): Record<string, ToolWaveStatus> {
  const snap: Record<string, ToolWaveStatus> = {};
  for (const tool of tools) snap[tool.id] = tool.status;
  return snap;
}

/**
 * 跟踪并行工具的开始/结束，按队列消费成轮播当前项。
 *
 * @param tools 当前工具快照
 * @param enabled 折叠且并行时才排队
 * @returns 当前展示的工具 id，以及是否仍需占据折叠位
 */
export function useToolWaveQueue(
  tools: readonly ToolWaveSnapshot[],
  enabled: boolean
): { currentId: string | null; busy: boolean } {
  const reducedMotion = useReducedMotion();
  const prevRef = useRef<Record<string, ToolWaveStatus>>({});
  const processedKeyRef = useRef("");
  const queueRef = useRef<ToolWaveEvent[]>([]);
  const holdRef = useRef<number | null>(null);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const key = tools.map((tool) => `${tool.id}:${tool.status}`).join(",");

  if (!enabled) {
    prevRef.current = snapshotToolWaveStatus(tools);
    processedKeyRef.current = key;
    queueRef.current = [];
  } else if (key !== processedKeyRef.current) {
    const incoming = diffToolWaveEvents(prevRef.current, tools);
    prevRef.current = snapshotToolWaveStatus(tools);
    processedKeyRef.current = key;
    if (incoming.length > 0) {
      queueRef.current = enqueueToolWaveEvents(queueRef.current, incoming);
    }
  }

  const running = tools.some((tool) => isToolWaveActive(tool.status));
  const busy = enabled && (running || playing || queueRef.current.length > 0);

  useEffect(() => {
    if (!enabled) {
      setPlaying(false);
      if (holdRef.current !== null) {
        window.clearTimeout(holdRef.current);
        holdRef.current = null;
      }
      return;
    }
    if (playing || holdRef.current !== null) return;
    if (queueRef.current.length === 0) return;

    if (reducedMotion) {
      const last = queueRef.current[queueRef.current.length - 1];
      queueRef.current = [];
      setCurrentId(last.id);
      return;
    }

    const next = queueRef.current.shift();
    if (!next) return;
    setCurrentId(next.id);
    setPlaying(true);
    holdRef.current = window.setTimeout(() => {
      holdRef.current = null;
      setPlaying(false);
    }, WAVE_SLIDE_HOLD_MS);
  }, [enabled, key, playing, reducedMotion]);

  useEffect(() => () => {
    if (holdRef.current !== null) window.clearTimeout(holdRef.current);
  }, []);

  return { currentId, busy };
}
