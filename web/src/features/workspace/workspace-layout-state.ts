export const WORKSPACE_LAYOUT_STORAGE_KEY = "sai.workspace-layout";

export type WorkspaceLayoutState = {
  chatOpen: boolean;
  workspaceOpen: boolean;
  workspaceWidth: number;
  workspaceMaximized: boolean;
  terminalOpen: boolean;
  terminalHeight: number;
  swapped: boolean;
};

const DEFAULT_WORKSPACE_WIDTH = 520;
const MIN_WORKSPACE_WIDTH = 320;
const RESERVED_WORKSPACE_WIDTH = 610;
const RESERVED_WORKBENCH_WIDTH = 367;
const DEFAULT_TERMINAL_HEIGHT = 280;
const MIN_TERMINAL_HEIGHT = 150;

/**
 * 约束右侧工作区宽度，避免挤压会话栏和聊天区。
 *
 * @param width 请求设置的工作区宽度
 * @param viewportWidth 当前视口宽度
 * @returns 经过边界约束的工作区宽度
 */
export function clampWorkspaceWidth(width: number, viewportWidth: number): number {
  // 1. 只保留会话栏与聊天区的最小可用空间，其余全部允许分配给工作区
  const maximum = Math.max(MIN_WORKSPACE_WIDTH, viewportWidth - RESERVED_WORKSPACE_WIDTH);
  return Math.min(Math.max(width, MIN_WORKSPACE_WIDTH), maximum);
}

/**
 * 按工作台实际宽度约束编辑区域宽度。
 *
 * @param width 请求设置的工作区宽度
 * @param workbenchWidth 不含会话侧栏的工作台宽度
 * @returns 经过聊天区最小宽度约束的工作区宽度
 */
export function clampWorkspaceWidthForWorkbench(width: number, workbenchWidth: number): number {
  const maximum = Math.max(MIN_WORKSPACE_WIDTH, workbenchWidth - RESERVED_WORKBENCH_WIDTH);
  return Math.min(Math.max(width, MIN_WORKSPACE_WIDTH), maximum);
}

/**
 * 约束底部终端高度。
 *
 * @param height 请求设置的终端高度
 * @param viewportHeight 当前视口高度
 * @returns 经过边界约束的终端高度
 */
export function clampTerminalHeight(height: number, viewportHeight: number): number {
  return Math.min(Math.max(height, MIN_TERMINAL_HEIGHT), Math.max(MIN_TERMINAL_HEIGHT, viewportHeight * 0.65));
}

/**
 * 解析持久化布局状态，非法内容回退到默认值。
 *
 * @param serialized 本地存储中的序列化内容
 * @param viewportWidth 当前视口宽度
 * @returns 可直接使用的布局状态
 */
export function parseWorkspaceLayout(serialized: string | null, viewportWidth: number, viewportHeight = 900): WorkspaceLayoutState {
  if (!serialized) return createDefaultWorkspaceLayout(viewportWidth, viewportHeight);
  try {
    const value = JSON.parse(serialized) as Partial<WorkspaceLayoutState>;
    return {
      chatOpen: value.chatOpen !== false,
      workspaceOpen: value.workspaceOpen !== false,
      workspaceWidth: clampWorkspaceWidth(Number(value.workspaceWidth) || DEFAULT_WORKSPACE_WIDTH, viewportWidth),
      workspaceMaximized: value.workspaceMaximized === true,
      terminalOpen: value.terminalOpen === true,
      terminalHeight: clampTerminalHeight(Number(value.terminalHeight) || DEFAULT_TERMINAL_HEIGHT, viewportHeight),
      swapped: value.swapped === true
    };
  } catch {
    return createDefaultWorkspaceLayout(viewportWidth, viewportHeight);
  }
}

/**
 * 创建默认工作区布局状态。
 *
 * @param viewportWidth 当前视口宽度
 * @returns 默认布局状态
 */
export function createDefaultWorkspaceLayout(viewportWidth: number, viewportHeight = 900): WorkspaceLayoutState {
  return {
    chatOpen: true,
    workspaceOpen: true,
    workspaceWidth: clampWorkspaceWidth(DEFAULT_WORKSPACE_WIDTH, viewportWidth),
    workspaceMaximized: false,
    terminalOpen: false,
    terminalHeight: clampTerminalHeight(DEFAULT_TERMINAL_HEIGHT, viewportHeight),
    swapped: false
  };
}
