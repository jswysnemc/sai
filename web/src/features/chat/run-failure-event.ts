import type { WebEvent } from "../../api/contracts";

/**
 * 构造仅供前端状态归并使用的运行失败事件。
 *
 * @param runId 运行标识
 * @param sessionId 会话标识
 * @param message 面向用户的错误摘要
 * @param detail 原始错误详情
 * @returns 与服务端终态事件结构一致的失败事件
 */
export function runFailureEvent(runId: string, sessionId: string | undefined, message: string, detail: string): WebEvent {
  return {
    sequence: 0,
    run_id: runId,
    workspace_id: "",
    session_id: sessionId ?? "",
    timestamp: new Date().toISOString(),
    type: "run.failed",
    payload: { message, detail }
  };
}

