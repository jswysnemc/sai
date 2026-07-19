import type { WorkspacePanelTab } from "../workspace/workspace-tab";

/**
 * 将终端标签加入工作区标签列表。
 *
 * @param tabs 当前工作区标签
 * @param terminalTab 待加入的终端标签
 * @returns 更新后的标签列表
 */
export function ensureTerminalTab(
  tabs: WorkspacePanelTab[],
  terminalTab: WorkspacePanelTab
): WorkspacePanelTab[] {
  if (terminalTab.type === "terminal" && terminalTab.terminalId) {
    const existing = tabs.find((tab) => (
      tab.type === "terminal" && tab.terminalId === terminalTab.terminalId
    ));
    if (existing) return tabs;
  }
  return [...tabs, terminalTab];
}
