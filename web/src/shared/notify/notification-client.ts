import { apiRequest } from "../../api/client";
import type { Locale } from "../../features/i18n/locale";

/** 【通知接口】【展示契约】服务端纯策略返回的内容与独立投递开关。 */
export type NotificationMessage = {
  title: string;
  body: string;
  desktop: boolean;
  sound: boolean;
};

export type ReplyStatus = "completed" | "interrupted" | "failed";

/**
 * 【通知接口】【计划读取】请求当前 Lua 策略，不在浏览器重复维护开关与文案。
 * @param status 宿主已经报告的结束状态
 * @param locale 当前界面语言
 * @param signal 本次投递的取消信号
 * @returns 经过结构校验的通知数据，失败由调用方隔离
 */
export async function fetchNotificationPlan(
  status: ReplyStatus,
  locale: Locale,
  signal: AbortSignal
): Promise<NotificationMessage[]> {
  const response = await apiRequest<{ notifications: unknown }>("/api/notifications/plan", {
    method: "POST",
    body: JSON.stringify({ status, locale }),
    signal
  });
  const notifications = response.notifications;
  if (!Array.isArray(notifications) || notifications.length > 8 || !notifications.every(isNotification)) {
    throw new Error("Invalid notification plan");
  }
  return notifications;
}

/**
 * 【通知接口】【响应校验】拒绝不完整数据，禁止把缺失开关默认为启用。
 * @param value 单条响应数据
 * @returns 是否符合通知契约
 */
function isNotification(value: unknown): value is NotificationMessage {
  if (value === null || typeof value !== "object") return false;
  const item = value as Record<string, unknown>;
  return typeof item.title === "string"
    && typeof item.body === "string"
    && typeof item.desktop === "boolean"
    && typeof item.sound === "boolean";
}
