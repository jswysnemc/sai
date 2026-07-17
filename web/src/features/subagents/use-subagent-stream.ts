import { useEffect, useState } from "react";
import type { Subagent, SubagentTimelineEntry } from "../../api/contracts";
import { mergeSubagentSnapshot } from "./subagent-message-parts";

type SubagentStreamEvent = {
  sequence: number;
  timestamp: string;
  snapshot: Subagent;
  timeline: SubagentTimelineEntry[];
};

/**
 * 订阅子智能体原生 SSE 详情流。
 *
 * @param initial 列表接口提供的初始子智能体快照
 * @returns 最新快照、时间线和事件时间
 */
export function useSubagentStream(initial: Subagent) {
  const [snapshot, setSnapshot] = useState(initial);
  const [timeline, setTimeline] = useState<SubagentTimelineEntry[]>([]);
  const [timestamp, setTimestamp] = useState("");

  useEffect(() => {
    setSnapshot(initial);
    setTimeline([]);
    setTimestamp("");
    const source = new EventSource(`/api/subagents/${encodeURIComponent(initial.id)}/events`);
    const handle = (message: MessageEvent<string>) => {
      const event = JSON.parse(message.data) as SubagentStreamEvent;
      setSnapshot((current) => mergeSubagentSnapshot(current, event.snapshot));
      setTimeline(event.timeline);
      setTimestamp(event.timestamp);
    };
    source.addEventListener("subagent.updated", handle as EventListener);
    return () => source.close();
  }, [initial.id]);

  return { snapshot, timeline, timestamp };
}
