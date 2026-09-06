import type { WorkspacePanelTab } from "./workspace-tab";

/**
 * 为同名文件计算最短且能区分彼此的目录后缀。
 * @param tabs 当前打开的工作区标签
 * @returns 标签标识与目录说明的映射；没有同名文件时不附加说明
 */
export function workspaceTabDirectoryLabels(tabs: WorkspacePanelTab[]): Map<string, string> {
  const labels = new Map<string, string>();
  const files = tabs.filter((tab) => tab.path).map((tab) => ({
    tab,
    parts: tab.path!.replaceAll("\\", "/").split("/").filter(Boolean),
  }));

  // 1. 【工作台】【标签导航】只比较文件名相同、实际路径不同的标签
  for (const { tab, parts } of files) {
    const peers = files.filter((file) => file.parts.at(-1) === parts.at(-1)
      && file.parts.join("/") !== parts.join("/"));
    if (!peers.length) continue;
    const directory = parts.slice(0, -1);
    if (!directory.length) {
      labels.set(tab.id, "./");
      continue;
    }

    // 2. 【工作台】【标签导航】从末级目录向上补足，直到能够区分同名文件
    for (let depth = 1; depth <= directory.length; depth += 1) {
      const suffix = directory.slice(-depth).join("/");
      const unique = peers.every((file) => file.parts.slice(0, -1).slice(-depth).join("/") !== suffix);
      if (unique || depth === directory.length) {
        labels.set(tab.id, suffix);
        break;
      }
    }
  }
  return labels;
}
