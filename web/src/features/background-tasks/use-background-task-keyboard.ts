import type { KeyboardEvent } from "react";
import type { BackgroundTask } from "../../api/contracts";
import type { BackgroundTaskFilter } from "./background-task-utils";

type TaskKeyboardOptions = {
  tasks: BackgroundTask[];
  selected?: BackgroundTask;
  busy: boolean;
  onSelect: (id: string) => void;
  onFilter: (filter: BackgroundTaskFilter) => void;
  onLogs: () => void;
  onStop: (force: boolean) => void;
  onCleanup: () => void;
  onRefresh: () => void;
};

/**
 * 管理列表快捷键，不拦截输入框、确认弹窗及浏览器 Tab 导航。
 * @param options 可见任务、操作状态和回调
 * @returns 面板键盘事件处理函数
 */
export function useBackgroundTaskKeyboard(options: TaskKeyboardOptions) {
  return (event: KeyboardEvent<HTMLElement>) => {
    if (event.ctrlKey || event.metaKey || event.altKey || !(event.target instanceof HTMLElement)) return;
    if (!event.currentTarget.contains(event.target) || event.target.closest("input, textarea, select, [contenteditable=true], [role=dialog], [role=radiogroup]")) return;
    const index = options.tasks.findIndex((task) => task.id === options.selected?.id);
    let next = index;
    if (["ArrowUp", "k"].includes(event.key)) next = Math.max(0, index - 1);
    else if (["ArrowDown", "j"].includes(event.key)) next = Math.min(options.tasks.length - 1, index + 1);
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = options.tasks.length - 1;
    else if (event.key === "o" || (event.key === "Enter" && event.target.closest(".background-task-select"))) options.onLogs();
    else if (["1", "2", "3"].includes(event.key)) options.onFilter((["all", "running", "finished"] as const)[Number(event.key) - 1]);
    else if (event.key === "r") options.onRefresh();
    else if (event.key === "s" && !options.busy) options.onStop(false);
    else if (event.key === "f" && !options.busy) options.onStop(true);
    else if (event.key === "x" && !options.busy) options.onCleanup();
    else return;
    event.preventDefault();
    if (next !== index && options.tasks[next]) options.onSelect(options.tasks[next].id);
  };
}
