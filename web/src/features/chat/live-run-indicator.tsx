import type { LiveRunState } from "./run-event-reducer";
import "./live-run-indicator.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 在实时助手消息末尾展示当前运行阶段。
 *
 * @param props 当前运行阶段
 * @returns 对应阶段的局部运行指示器，空闲时不渲染
 */
export function LiveRunIndicator({ status }: { status: LiveRunState["status"] }) {
  const { t } = useI18n();
  if (status === "idle") return null;
  const labels: Record<Exclude<LiveRunState["status"], "idle">, string> = {
    queued: t("Queued for this session", "已加入会话队列"),
    waiting_response: t("Waiting for model response", "等待模型响应"),
    waiting_permission: t("Waiting for permission decision", "等待权限决定"),
    waiting_question: t("Waiting for your answer", "等待你的回答"),
    thinking: t("Organizing thoughts", "正在整理思路"),
    working: t("Working on the task", "正在执行任务"),
    compacting: t("Compacting context", "正在压缩上下文")
  };
  return (
    <div className={`live-run-indicator ${status}`} role="status" aria-live="polite">
      <span className="live-run-motion" aria-hidden="true"><i /><i /><i /></span>
      <span>{labels[status]}</span>
    </div>
  );
}
