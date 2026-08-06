/** 被动打开的单文件 Diff 数据。 */
export type WorkspacePassiveDiff = {
  path: string;
  source: string;
  title?: string;
};

/** 请求打开右侧空侧栏的事件名。 */
export const OPEN_WORKSPACE_SIDEBAR_EVENT = "sai:open-workspace-sidebar";

/** 请求在右侧侧栏显示具体 Diff 的事件名。 */
export const OPEN_WORKSPACE_DIFF_EVENT = "sai:open-workspace-diff";

/**
 * 请求右侧侧栏显示一项具体 Diff。
 *
 * 参数:
 * - `detail`: Diff 路径、补丁和可选标题
 *
 * 返回:
 * - 无返回值
 */
export function openWorkspaceDiff(detail: WorkspacePassiveDiff): void {
  window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_DIFF_EVENT, { detail }));
}
