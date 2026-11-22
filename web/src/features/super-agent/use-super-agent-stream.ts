import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";

const SUPER_AGENT_EVENT_TYPES = [
  "group.created",
  "group.updated",
  "group.deleted",
  "message.created",
  "trigger.recorded",
  "leader.thinking",
  "leader.idle",
  "runner.created",
  "runner.progress",
  "runner.messaged",
  "runner.stopped",
  "task.dispatched",
  "task.retried",
  "task.stopped",
  "report.submitted",
  "delivery.completed",
  "subscription.updated",
  "subscription.removed",
  "channel.bound",
  "channel.unbound",
  "watchdog.event",
  "config.updated"
];

/**
 * 【超级智能体】【实时事件流】订阅服务端 SSE 增量事件，驱动群聊、Runner、任务与渠道状态实时刷新。
 *
 * @param activeGroupId 当前选中的群组标识
 * @returns 无返回值
 */
export function useSuperAgentStream(activeGroupId?: string) {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (typeof EventSource === "undefined") {
      return;
    }
    const url = activeGroupId
      ? `/api/super-agent/stream?group_id=${encodeURIComponent(activeGroupId)}`
      : "/api/super-agent/stream";
    const source = new EventSource(url);
    let timer: number | null = null;

    // 1. 节流合并短时间内的批量事件，避免 Runner 高频进度造成重复拉取
    const scheduleRefresh = () => {
      if (timer !== null) {
        return;
      }
      timer = window.setTimeout(() => {
        timer = null;
        void queryClient.invalidateQueries({ queryKey: ["super-agent-overview"] });
        if (activeGroupId) {
          void queryClient.invalidateQueries({ queryKey: ["super-agent-group", activeGroupId] });
        }
      }, 150);
    };

    for (const kind of SUPER_AGENT_EVENT_TYPES) {
      source.addEventListener(kind, scheduleRefresh);
    }
    source.onmessage = scheduleRefresh;

    return () => {
      if (timer !== null) {
        window.clearTimeout(timer);
      }
      source.close();
    };
  }, [activeGroupId, queryClient]);
}
