export type PaneTab = "files" | "diff" | "terminal" | "tasks" | "subagents";

export type WorkspacePanelTab = {
  id: string;
  type: PaneTab;
  title: string;
  path?: string;
  /** 终端会话 ID，仅 terminal 类型使用。 */
  terminalId?: string;
  closable: boolean;
};

/**
 * 创建工作区面板标签。
 *
 * @param type 面板类型
 * @param options 标题、文件路径或终端会话
 * @returns 工作区标签
 */
export function createWorkspacePanelTab(
  type: PaneTab,
  options: { title?: string; path?: string; terminalId?: string; closable?: boolean } = {}
): WorkspacePanelTab {
  const path = options.path;
  if (type === "files") {
    const title = options.title ?? (path ? path.split("/").filter(Boolean).at(-1) ?? path : "编辑器");
    return {
      id: path ? `file:${path}` : `files:${crypto.randomUUID()}`,
      type,
      title,
      path,
      closable: options.closable ?? true
    };
  }
  if (type === "terminal") {
    const terminalId = options.terminalId ?? crypto.randomUUID();
    return {
      id: `terminal:${terminalId}`,
      type,
      title: options.title ?? "终端",
      terminalId,
      closable: options.closable ?? true
    };
  }
  const defaults: Record<Exclude<PaneTab, "files" | "terminal">, string> = {
    diff: "Git",
    tasks: "后台任务",
    subagents: "子智能体"
  };
  return {
    id: `${type}:${crypto.randomUUID()}`,
    type,
    title: options.title ?? defaults[type],
    closable: options.closable ?? true
  };
}

/**
 * 返回指定类型面板的默认标题。
 *
 * @param type 面板类型
 * @returns 标题
 */
export function paneTabLabel(type: PaneTab): string {
  return {
    files: "编辑器",
    diff: "Git",
    terminal: "终端",
    tasks: "后台任务",
    subagents: "子智能体"
  }[type];
}
