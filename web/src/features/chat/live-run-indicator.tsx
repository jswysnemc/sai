import { useEffect, useState } from "react";
import type { LiveRunState } from "./run-event-reducer";
import "./live-run-indicator.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 在实时助手消息末尾展示当前运行阶段与本轮已用时长。
 *
 * 重连态与其它忙碌态区分：文案带 attempt/max，并用告警色脉冲点，
 * 避免与「正在执行任务」混淆（对齐 Codex 的 StreamError 状态指示，但落在消息内指示器而非底部栏）。
 *
 * @param props status 为运行阶段，startedAtMs 为本轮开始时间
 * @returns 对应阶段的局部运行指示器，空闲时不渲染
 */
export function LiveRunIndicator({
  status,
  startedAtMs,
  reconnectAttempt = null,
  reconnectMaxAttempts = null
}: {
  status: LiveRunState["status"];
  startedAtMs?: number | null;
  reconnectAttempt?: number | null;
  reconnectMaxAttempts?: number | null;
}) {
  const { t, locale } = useI18n();
  const [nowMs, setNowMs] = useState(() => Date.now());

  // 1. 运行中每秒刷新已用时长
  useEffect(() => {
    if (status === "idle" || !startedAtMs) return;
    setNowMs(Date.now());
    const timer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [startedAtMs, status]);

  if (status === "idle") return null;
  const reconnectProgress =
    status === "reconnecting" && reconnectAttempt != null && reconnectMaxAttempts != null
      ? ` ${reconnectAttempt}/${reconnectMaxAttempts}`
      : "";
  const labels: Record<Exclude<LiveRunState["status"], "idle">, string> = {
    queued: t("Queued for this session", "已加入会话队列"),
    waiting_response: t("Waiting for model response", "等待模型响应"),
    waiting_external: t("Waiting for background work", "等待后台工作"),
    waiting_permission: t("Waiting for permission decision", "等待权限决定"),
    waiting_question: t("Waiting for your answer", "等待你的回答"),
    waiting_ssh_secret: t("Waiting for secure SSH input", "等待 SSH 安全输入"),
    thinking: t("Organizing thoughts", "正在整理思路"),
    working: t("Working on the task", "正在执行任务"),
    compacting: t("Compacting context", "正在压缩上下文"),
    reconnecting: `${t("Reconnecting…", "正在重连…")}${reconnectProgress}`
  };
  const elapsed = startedAtMs
    ? formatTurnElapsed(Math.max(0, nowMs - startedAtMs), locale.startsWith("zh"))
    : null;
  // 2. 等待用户决定的两个阶段球在用户这边，不做流动动效，避免暗示系统正在推进
  //    重连用独立 tone：仍脉冲，但换成告警色，和普通 busy 扫光区分开
  const tone = AWAITING_USER_STATUSES.has(status)
    ? "awaiting"
    : status === "reconnecting"
      ? "reconnecting"
      : "busy";
  return (
    <div className={`live-run-indicator ${status} ${tone}`} role="status" aria-live="polite">
      <span className="live-run-dot" aria-hidden="true" />
      <span className="live-run-label">{labels[status]}</span>
      {elapsed ? <span className="live-run-elapsed tnum">{elapsed}</span> : null}
    </div>
  );
}

/** 等待用户操作的阶段，指示器保持静态 */
const AWAITING_USER_STATUSES = new Set<LiveRunState["status"]>([
  "waiting_permission",
  "waiting_question",
  "waiting_ssh_secret"
]);

/**
 * 格式化本轮已用时长。
 *
 * @param elapsedMs 已用毫秒
 * @param zh 是否中文
 * @returns 如 `1分20秒` 或 `1m 20s`
 */
export function formatTurnElapsed(elapsedMs: number, zh: boolean): string {
  const totalSecs = Math.floor(elapsedMs / 1_000);
  if (totalSecs < 60) {
    return zh ? `${totalSecs}秒` : `${totalSecs}s`;
  }
  const minutes = Math.floor(totalSecs / 60);
  const seconds = totalSecs % 60;
  if (minutes < 60) {
    return zh ? `${minutes}分${seconds}秒` : `${minutes}m ${seconds}s`;
  }
  const hours = Math.floor(minutes / 60);
  const remainMinutes = minutes % 60;
  return zh
    ? `${hours}小时${remainMinutes}分${seconds}秒`
    : `${hours}h ${remainMinutes}m ${seconds}s`;
}
