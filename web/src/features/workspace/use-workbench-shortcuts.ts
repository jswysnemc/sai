import { useEffect } from "react";
import { handleWorkbenchShortcut } from "./workbench-shortcuts";

/**
 * 注册工作台快捷键，弹窗打开时将键盘操作交给弹窗。
 * @returns 无返回值
 */
export function useWorkbenchShortcuts(): void {
  useEffect(() => {
    /**
     * 根据按键发送动作，并阻止对应浏览器默认行为。
     * @param event 当前页面的键盘事件
     * @returns 无返回值
     */
    const handleKeyDown = (event: KeyboardEvent) => {
      handleWorkbenchShortcut(event);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);
}
