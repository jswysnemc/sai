import type { Subagent, SubagentTimelineEntry } from "../../api/contracts";
import type { LiveMessagePart, ToolLifecycle } from "../chat/run-event-reducer";

/**
 * 将子智能体时间线转换为主对话统一消息部件。
 *
 * @param entries 子智能体时间线
 * @param running 子智能体是否仍在运行
 * @param timestamp 最近事件时间
 * @returns 可交给 MessageParts 渲染的有序部件
 */
export function subagentMessageParts(
  entries: SubagentTimelineEntry[],
  running: boolean,
  timestamp = ""
): LiveMessagePart[] {
  return entries.map((entry, index) => {
    if (entry.kind === "reasoning") {
      return {
        id: `subagent-reasoning-${index}`,
        type: "reasoning" as const,
        source: entry.text,
        startedAt: timestamp,
        endedAt: running && index === entries.length - 1 ? undefined : timestamp
      };
    }
    if (entry.kind === "text") {
      return { id: `subagent-text-${index}`, type: "text" as const, source: entry.text };
    }
    return {
      id: `subagent-tool-${entry.step}-${index}`,
      type: "tool" as const,
      tool: subagentToolLifecycle(entry, running)
    };
  });
}

/**
 * 将子智能体工具条目转换为主对话工具生命周期。
 *
 * @param entry 子智能体工具时间线条目
 * @param running 子智能体是否仍在运行
 * @returns 主对话工具生命周期
 */
function subagentToolLifecycle(
  entry: Extract<SubagentTimelineEntry, { kind: "tool" }>,
  running: boolean
): ToolLifecycle {
  return {
    id: `subagent-tool-${entry.step}`,
    name: entry.name,
    argumentsPreview: entry.args_preview,
    arguments: entry.args_preview,
    progress: entry.ok == null && running ? "正在执行" : "",
    output: entry.output_preview ?? "",
    status: entry.ok == null ? (running ? "running" : "failed") : entry.ok ? "completed" : "failed"
  };
}

/**
 * 合并列表快照与流事件快照，保留列表中已经存在的字段。
 *
 * @param current 当前子智能体快照
 * @param incoming 流事件中的子智能体快照
 * @returns 合并后的快照
 */
export function mergeSubagentSnapshot(current: Subagent, incoming: Subagent): Subagent {
  return { ...current, ...incoming };
}
