import type { LiveRunState, ToolLifecycle } from "./run-event-reducer";

/**
 * 【上下文】【运行状态】判断是否仍有正在生成摘要或执行压缩的调用。
 * @param tools 当前运行的工具列表
 * @returns 是否存在未结束的压缩调用
 */
export function hasActiveContextCompression(tools: ToolLifecycle[]): boolean {
  return tools.some((tool) => tool.name === "compress_context"
    && (tool.status === "preparing" || tool.status === "running"));
}

/**
 * 【上下文】【运行状态】随工具事件更新压缩状态，结束后恢复当前工具活动状态。
 * @param state 更新工具后的运行状态
 * @returns 与未完成工具一致的运行状态
 */
export function withContextCompressionStatus(state: LiveRunState): LiveRunState {
  if (state.completed) return state;
  if (hasActiveContextCompression(state.tools) && ["working", "waiting_response", "thinking", "compacting"].includes(state.status)) return { ...state, status: "compacting" };
  if (state.status !== "compacting" || state.parts.some((part) => part.type === "compaction" && part.status === "running")) return state;
  const working = state.tools.some((tool) => tool.status === "preparing" || tool.status === "running");
  return { ...state, status: working ? "working" : "waiting_response" };
}

/**
 * 【上下文】【中断反馈】运行结束但尚无结果时，将未完成压缩显示为失败。
 * @param state 已结束的运行状态
 * @param message 结束原因
 * @returns 不再保留运行中压缩指示的状态
 */
export function finishPendingContextCompression(state: LiveRunState, message: string): LiveRunState {
  const pending = state.parts.some((part) => part.type === "tool" && hasActiveContextCompression([part.tool]));
  if (!pending) return state;
  const tools = state.tools.map((tool): ToolLifecycle => tool.name === "compress_context"
    && (tool.status === "preparing" || tool.status === "running")
    ? { ...tool, status: "failed", output: message } : tool);
  return {
    ...state,
    tools,
    parts: state.parts.map((part) => part.type === "tool"
      ? { ...part, tool: tools.find((tool) => tool.id === part.tool.id) ?? part.tool } : part)
  };
}
