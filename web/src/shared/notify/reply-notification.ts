import type { RunInfo, WebEvent } from "../../api/contracts";
import type { Locale } from "../../features/i18n/locale";
import { fetchNotificationPlan, type ReplyStatus } from "./notification-client";
import { deliverNotification } from "./notification-delivery";

const TERMINAL_STATUS = new Map<string, ReplyStatus>([
  ["run.completed", "completed"],
  ["run.interrupted", "interrupted"],
  ["run.failed", "failed"]
]);
const RUN_ENTRIES = new Set(["run.started", "run.queued", "run.dequeued"]);
const TRACKED_RUN_LIMIT = 512;

type Dependencies = {
  plan: typeof fetchNotificationPlan;
  deliver: typeof deliverNotification;
};

/**
 * 【答复通知】【消费范围】管理一个已打开会话的通知去重与请求取消。
 * @param workspaceId 当前工作区
 * @param sessionId 当前会话
 * @param dependencies 可替换的策略接口与平台投递，用于验证真实调度行为
 * @returns 事件消费、活动运行登记与销毁方法
 */
export function createReplyNotifier(
  workspaceId: string,
  sessionId: string,
  dependencies: Dependencies = { plan: fetchNotificationPlan, deliver: deliverNotification }
) {
  const consumed = new Set<string>();
  const active = new Set<string>();
  const pending = new Set<AbortController>();
  let lastSequence = 0;
  let disposed = false;

  /**
   * 【答复通知】【活动登记】让重连后补发的真实完成仍能通知一次。
   * @param runs 已从宿主取得的当前活动运行
   * @returns 无；其他工作区或会话不会进入当前消费范围
   */
  function trackActiveRuns(runs: RunInfo[]): void {
    if (disposed) return;
    for (const run of runs) {
      if (run.workspace_id === workspaceId && run.session_id === sessionId
        && (run.status === "running" || run.status === "queued") && !consumed.has(run.run_id)) {
        remember(active, run.run_id);
      }
    }
  }

  /**
   * 【答复通知】【终态消费】先登记运行再异步计算，重复失败回执和 SSE 重连不重复投递。
   * @param event 宿主事件
   * @param locale 当前界面语言
   * @returns 计划完成或取消后的 Promise；错误不会传播到事件流
   */
  async function accept(event: WebEvent, locale: Locale): Promise<void> {
    if (disposed || event.workspace_id !== workspaceId || event.session_id !== sessionId || !event.run_id) return;
    if (RUN_ENTRIES.has(event.type) && event.replayed !== true && !consumed.has(event.run_id)) {
      remember(active, event.run_id);
    }
    const status = TERMINAL_STATUS.get(event.type);
    if (!status || !Number.isSafeInteger(event.sequence) || event.sequence <= 0) return;
    if (consumed.has(event.run_id) || event.sequence <= lastSequence) return;
    lastSequence = event.sequence;
    const relevant = event.replayed !== true || active.has(event.run_id);
    active.delete(event.run_id);
    remember(consumed, event.run_id);
    if (!relevant) return;

    const controller = new AbortController();
    pending.add(controller);
    const timeout = setTimeout(() => controller.abort(), 5_000);
    try {
      const notifications = await dependencies.plan(status, locale, controller.signal);
      if (disposed || controller.signal.aborted) return;
      for (const notification of notifications) dependencies.deliver(notification, controller.signal);
    } catch {
      // 【答复通知】【失败隔离】保持事件消费结果，不重试已经消费的通知
    } finally {
      clearTimeout(timeout);
      // 【答复通知】【迟到权限】保留控制器直到销毁，权限提示可能晚于 HTTP 请求完成
      if (controller.signal.aborted) pending.delete(controller);
      if (pending.size > TRACKED_RUN_LIMIT) {
        const oldest = pending.values().next().value;
        oldest?.abort();
        if (oldest) pending.delete(oldest);
      }
    }
  }

  /**
   * 【答复通知】【范围销毁】会话切换和组件卸载时取消未完成请求及迟到的权限回调。
   * @returns 无
   */
  function dispose(): void {
    disposed = true;
    for (const controller of pending) controller.abort();
    pending.clear();
    active.clear();
    consumed.clear();
  }

  return { accept, trackActiveRuns, dispose };
}

/**
 * 【答复通知】【内存边界】仅保存有限数量的近期运行标识。
 * @param entries 目标集合
 * @param id 要登记的运行标识
 * @returns 无
 */
function remember(entries: Set<string>, id: string): void {
  entries.add(id);
  if (entries.size > TRACKED_RUN_LIMIT) {
    const oldest = entries.values().next().value;
    if (oldest) entries.delete(oldest);
  }
}
