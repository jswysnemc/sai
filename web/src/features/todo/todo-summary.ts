import type { TodoItem, TodoStatus } from "../../api/contracts";

export type TodoSummary = {
  total: number;
  completed: number;
  inProgress: number;
  pending: number;
  cancelled: number;
  activeText: string | null;
  ratio: number;
  allDone: boolean;
};

const activeStatusPriority: TodoStatus[] = ["in_progress", "pending"];

/**
 * 汇总 TODO 清单的进度信息。
 *
 * @param items TODO 项列表
 * @returns 进度摘要
 */
export function summarizeTodos(items: TodoItem[]): TodoSummary {
  const counts: Record<TodoStatus, number> = { pending: 0, in_progress: 0, completed: 0, cancelled: 0 };
  for (const item of items) counts[item.status] += 1;
  const total = items.length;
  const finished = counts.completed + counts.cancelled;
  return {
    total,
    completed: counts.completed,
    inProgress: counts.in_progress,
    pending: counts.pending,
    cancelled: counts.cancelled,
    activeText: findActiveText(items),
    ratio: total > 0 ? finished / total : 0,
    allDone: total > 0 && finished === total
  };
}

/**
 * 找出当前应展示的活动项文本:优先进行中,其次待处理。
 *
 * @param items TODO 项列表
 * @returns 活动项文本,全部完成时为 null
 */
function findActiveText(items: TodoItem[]): string | null {
  for (const status of activeStatusPriority) {
    const match = items.find((item) => item.status === status);
    if (match) return match.text;
  }
  return null;
}
