import { useEffect, useState } from "react";
import {
  clampWorkspaceWidth,
  clampWorkspaceWidthForWorkbench,
  clampTerminalHeight,
  parseWorkspaceLayout,
  WORKSPACE_LAYOUT_STORAGE_KEY,
  type WorkspaceLayoutState
} from "./workspace-layout-state";

/**
 * 管理右侧工作区的显隐、宽度和本地持久化。
 *
 * @returns 工作区布局状态与操作方法
 */
export function useWorkspaceLayout() {
  const [layout, setLayout] = useState<WorkspaceLayoutState>(() => parseWorkspaceLayout(
    window.localStorage.getItem(WORKSPACE_LAYOUT_STORAGE_KEY),
    window.innerWidth,
    window.innerHeight
  ));

  useEffect(() => {
    window.localStorage.setItem(WORKSPACE_LAYOUT_STORAGE_KEY, JSON.stringify(layout));
  }, [layout]);

  useEffect(() => {
    /** 根据视口变化重新约束工作区宽度。 */
    const handleResize = () => {
      setLayout((current) => ({
        ...current,
        workspaceWidth: clampWorkspaceWidth(current.workspaceWidth, window.innerWidth),
        terminalHeight: clampTerminalHeight(current.terminalHeight, window.innerHeight)
      }));
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  /** 打开右侧工作区。 */
  const openWorkspace = () => setLayout((current) => ({ ...current, workspaceOpen: true }));

  /** 关闭右侧工作区。 */
  const closeWorkspace = () => setLayout((current) => ({ ...current, workspaceOpen: false }));

  /** 切换聊天主渲染区。 */
  const toggleChat = () => setLayout((current) => ({ ...current, chatOpen: !current.chatOpen, workspaceMaximized: false }));

  /** 切换编辑工作区最大化状态。 */
  const toggleWorkspaceMaximized = () => setLayout((current) => ({ ...current, workspaceOpen: true, workspaceMaximized: !current.workspaceMaximized }));

  /** 打开底部终端。 */
  const openTerminal = () => setLayout((current) => ({ ...current, terminalOpen: true }));

  /** 关闭底部终端。 */
  const closeTerminal = () => setLayout((current) => ({ ...current, terminalOpen: false }));

  /** 切换底部终端显隐。 */
  const toggleTerminal = () => setLayout((current) => ({ ...current, terminalOpen: !current.terminalOpen }));

  /** 左右调换聊天区与编辑工作区。 */
  const toggleSwapped = () => setLayout((current) => ({ ...current, swapped: !current.swapped }));

  /**
   * 更新右侧工作区宽度。
   *
   * @param width 请求设置的宽度
   * @param workbenchWidth 工作台实际宽度
   */
  const resizeWorkspace = (width: number, workbenchWidth?: number) => {
    setLayout((current) => ({
      ...current,
      workspaceWidth: workbenchWidth === undefined
        ? clampWorkspaceWidth(width, window.innerWidth)
        : clampWorkspaceWidthForWorkbench(width, workbenchWidth)
    }));
  };

  /**
   * 更新底部终端高度。
   *
   * @param height 请求设置的终端高度
   */
  const resizeTerminal = (height: number) => {
    setLayout((current) => ({ ...current, terminalHeight: clampTerminalHeight(height, window.innerHeight) }));
  };

  return { ...layout, openWorkspace, closeWorkspace, toggleChat, toggleWorkspaceMaximized, openTerminal, closeTerminal, toggleTerminal, toggleSwapped, resizeWorkspace, resizeTerminal };
}
