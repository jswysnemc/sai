export type WorkbenchCommand = "new-session" | "search" | "toggle-sidebar" | "toggle-terminal" | "open-files" | "open-changes" | "focus-composer";

export const WORKBENCH_COMMAND_EVENT = "sai:workbench-command";

type ShortcutEvent = Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey" | "isComposing" | "defaultPrevented" | "repeat">;

/**
 * 将组合键解析为工作台动作，忽略输入法组合、长按重复与已处理事件。
 * @param event 键盘按键及修饰键状态
 * @returns 匹配的动作，没有匹配时返回 null
 */
export function resolveWorkbenchShortcut(event: ShortcutEvent): WorkbenchCommand | null {
  if (event.defaultPrevented || event.isComposing || event.repeat || event.altKey || !(event.ctrlKey || event.metaKey)) return null;
  const key = event.key.toLowerCase();
  if (event.shiftKey) {
    if (key === "o") return "new-session";
    if (key === "p") return "search";
    if (key === "e") return "open-files";
    if (key === "g") return "open-changes";
    return null;
  }
  if (key === "k") return "search";
  if (key === "b") return "toggle-sidebar";
  if (key === "j") return "toggle-terminal";
  if (key === "i") return "focus-composer";
  return null;
}

/**
 * 【Web 工作台】【快捷操作】通知各功能模块执行同一工作台动作。
 * @param command 需要执行的动作
 * @returns 无返回值
 */
export function requestWorkbenchCommand(command: WorkbenchCommand): void {
  window.dispatchEvent(new CustomEvent<WorkbenchCommand>(WORKBENCH_COMMAND_EVENT, { detail: command }));
}

/**
 * 优先处理工作台组合键，弹层打开时交还控制权，长按时只抑制重复输入。
 * @param event 页面或终端收到的键盘事件
 * @returns 是否已经处理该组合键，供终端决定是否继续输入
 */
export function handleWorkbenchShortcut(event: KeyboardEvent): boolean {
  const command = resolveWorkbenchShortcut({
    key: event.key, ctrlKey: event.ctrlKey, metaKey: event.metaKey,
    altKey: event.altKey, shiftKey: event.shiftKey, isComposing: event.isComposing,
    defaultPrevented: event.defaultPrevented, repeat: false
  });
  if (!command || document.querySelector('[role="dialog"], [role="alertdialog"], [role="menu"]')) return false;
  event.preventDefault();
  if (!event.repeat) requestWorkbenchCommand(command);
  return true;
}
