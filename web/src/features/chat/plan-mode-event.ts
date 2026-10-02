import type { RunMode, WebEvent } from "../../api/contracts";

/**
 * 【计划模式】【状态同步】只从成功的实时计划工具结果读取模式，拒绝反馈不能退出 Plan。
 * @param event 当前会话运行事件
 * @returns 合法模式；无状态切换时返回 undefined
 */
export function planModeFromEvent(event: WebEvent): RunMode | undefined {
  if (event.replayed || event.type !== "tool.result" || event.payload.ok !== true) return;
  const name = event.payload.name;
  if (name !== "enter_plan_mode" && name !== "exit_plan_mode") return;
  if (typeof event.payload.output !== "string") return;
  try {
    const result = JSON.parse(event.payload.output) as Record<string, unknown>;
    if (name === "exit_plan_mode" && result.approved !== true) return;
    if (name === "enter_plan_mode") return result.mode === "plan" ? "plan" : undefined;
    switch (result.mode) {
      case "yolo": case "audited": return result.mode;
      case "auto-audit": return "auto_audit";
      default: return;
    }
  } catch { return; }
}
