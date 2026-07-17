import type { TodoStatus } from "../../api/contracts";

const todoStatusOrder: TodoStatus[] = ["pending", "in_progress", "completed", "cancelled"];

const todoStatusLabels: Record<TodoStatus, string> = {
  pending: "待处理",
  in_progress: "进行中",
  completed: "已完成",
  cancelled: "已取消"
};

/**
 * 返回待办状态的中文名称。
 *
 * @param status 待办状态
 * @returns 状态名称
 */
export function todoStatusLabel(status: TodoStatus): string {
  return todoStatusLabels[status];
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
