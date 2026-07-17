export type MobileWorkbenchPane = "chat" | "workspace" | "terminal";

export type MobileWorkbenchState = {
  sidebarOpen: boolean;
  pane: MobileWorkbenchPane;
};

export type MobileWorkbenchAction =
  | { type: "toggle-sidebar" }
  | { type: "close-sidebar" }
  | { type: "show-pane"; pane: MobileWorkbenchPane };

export const MOBILE_SIDEBAR_TOGGLE_EVENT = "sai:toggle-session-sidebar";
export const MOBILE_WORKBENCH_MEDIA_QUERY = "(max-width: 760px)";

export const initialMobileWorkbenchState: MobileWorkbenchState = {
  sidebarOpen: false,
  pane: "chat"
};

/**
 * 计算移动端工作台的抽屉和当前面板状态。
 *
 * @param state 当前移动端状态
 * @param action 用户触发的布局动作
 * @returns 更新后的移动端状态
 */
export function reduceMobileWorkbenchState(
  state: MobileWorkbenchState,
  action: MobileWorkbenchAction
): MobileWorkbenchState {
  if (action.type === "toggle-sidebar") return { ...state, sidebarOpen: !state.sidebarOpen };
  if (action.type === "close-sidebar") return state.sidebarOpen ? { ...state, sidebarOpen: false } : state;
  return { ...state, pane: action.pane, sidebarOpen: false };
}
