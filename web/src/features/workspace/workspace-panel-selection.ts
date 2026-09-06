import type { PaneTab, WorkspacePanelTab } from "./workspace-tab";

/**
 * 按目标文件与当前标签选择已有面板，避免类型同步将用户切回首个文件。
 * @param tabs 已经打开的面板标签
 * @param type 本次需要显示的面板类型
 * @param activeId 用户当前选中的标签标识
 * @param selectedFile 本次打开的文件路径，无文件时为 null
 * @returns 可复用的标签，没有匹配时返回 undefined
 */
export function selectExistingWorkspacePanel(
  tabs: WorkspacePanelTab[],
  type: PaneTab,
  activeId: string | null,
  selectedFile: string | null
): WorkspacePanelTab | undefined {
  const candidates = tabs.filter((tab) => tab.type === type);
  if (type === "files" && selectedFile) {
    const file = candidates.find((tab) => tab.path === selectedFile);
    if (file) return file;
  }
  return candidates.find((tab) => tab.id === activeId) ?? candidates[0];
}
