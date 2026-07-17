import type { LiveRunState } from "./run-event-reducer";
import "./live-run-indicator.css";

const labels: Record<Exclude<LiveRunState["status"], "idle">, string> = {
  queued: "已加入会话队列",
  waiting_response: "等待模型响应",
  waiting_permission: "等待权限决定",
  waiting_question: "等待你的回答",
  thinking: "正在整理思路",
  working: "正在执行任务",
  compacting: "正在压缩上下文"
};

/**
 * 在实时助手消息末尾展示当前运行阶段。
 *
 * @param props 当前运行阶段
 * @returns 对应阶段的局部运行指示器，空闲时不渲染
 */
export function LiveRunIndicator({ status }: { status: LiveRunState["status"] }) {
  if (status === "idle") return null;
  return (
    <div className={`live-run-indicator ${status}`} role="status" aria-live="polite">
      <span className="live-run-motion" aria-hidden="true"><i /><i /><i /></span>
      <span>{labels[status]}</span>
    </div>
  );
}
