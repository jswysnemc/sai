import type { WebEvent } from "../../api/contracts";

export type RunStreamReset = {
  through_sequence: number;
  events: WebEvent[];
  incomplete_run_ids: string[];
};

/**
 * 【会话同步】【快照校验】校验恢复载荷与事件归属，并统一标记为历史补发。
 * @param data SSE 原始载荷
 * @param workspaceId 当前工作区
 * @param sessionId 当前会话
 * @returns 可用于替换状态的快照；格式或归属无效时返回 null
 */
export function parseRunStreamReset(data: string, workspaceId: string, sessionId: string): RunStreamReset | null {
  try {
    const reset = JSON.parse(data) as RunStreamReset;
    if (!Number.isSafeInteger(reset.through_sequence) || reset.through_sequence < 0
      || !Array.isArray(reset.events) || !Array.isArray(reset.incomplete_run_ids)
      || !reset.incomplete_run_ids.every((id) => typeof id === "string")) return null;
    let previous = 0;
    for (const event of reset.events) {
      if (!event || event.workspace_id !== workspaceId || event.session_id !== sessionId
        || typeof event.run_id !== "string" || typeof event.type !== "string"
        || !event.payload || typeof event.payload !== "object"
        || !Number.isSafeInteger(event.sequence) || event.sequence <= previous
        || event.sequence > reset.through_sequence) return null;
      previous = event.sequence;
    }
    return { ...reset, events: reset.events.map((event) => ({ ...event, replayed: true })) };
  } catch {
    return null;
  }
}
