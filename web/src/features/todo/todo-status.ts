import type { TodoStatus } from "../../api/contracts";
import { text, type Locale } from "../i18n/locale";

const todoStatusOrder: TodoStatus[] = ["pending", "in_progress", "completed", "cancelled"];

/**
 * 返回待办状态的本地化名称。
 *
 * @param status 待办状态
 * @param locale 当前界面语言
 * @returns 状态名称
 */
export function todoStatusLabel(status: TodoStatus, locale: Locale = "zh-CN"): string {
  return {
    pending: text(locale, "Pending", "待处理"),
    in_progress: text(locale, "In progress", "进行中"),
    completed: text(locale, "Completed", "已完成"),
    cancelled: text(locale, "Cancelled", "已取消")
  }[status];
}

/**
 * 返回用户点击状态图标后应进入的状态。
 *
 * @param status 当前待办状态
 * @returns 下一个待办状态
 */
export function nextTodoStatus(status: TodoStatus): TodoStatus {
  const index = todoStatusOrder.indexOf(status);
  return todoStatusOrder[(index + 1) % todoStatusOrder.length];
}
