import { Activity, Bot, FileCode2, GitCompareArrows, SquareTerminal } from "lucide-react";
import type { PaneTab } from "./workspace-tab";

export type WorkspacePanelOption = {
  type: PaneTab;
  labelEn: string;
  labelZh: string;
  icon: typeof FileCode2;
};

/**
 * 工作区可打开面板的统一配置。
 *
 * 桌面端收起态 `+` 菜单、工作区标签栏 `+` 菜单和移动端聊天头部
 * `+` 菜单共用此列表，避免多处维护同一组面板选项。
 */
export const WORKSPACE_PANEL_OPTIONS: WorkspacePanelOption[] = [
  { type: "files", labelEn: "Editor", labelZh: "编辑器", icon: FileCode2 },
  { type: "diff", labelEn: "Git", labelZh: "Git", icon: GitCompareArrows },
  { type: "terminal", labelEn: "Terminal", labelZh: "终端", icon: SquareTerminal },
  { type: "tasks", labelEn: "Background tasks", labelZh: "后台任务", icon: Activity },
  { type: "subagents", labelEn: "Subagents", labelZh: "子智能体", icon: Bot }
];

/** 请求打开某个工作区面板的自定义事件名。 */
export const OPEN_WORKSPACE_PANEL_EVENT = "sai:open-workspace-panel";

/**
 * 返回当前语言下的面板标题。
 *
 * @param type 面板类型
 * @param t 双语文本选择方法
 * @returns 面板标题
 */
export function workspacePanelTitle(type: PaneTab, t: (en: string, zh: string) => string): string {
  const option = WORKSPACE_PANEL_OPTIONS.find((item) => item.type === type);
  return option ? t(option.labelEn, option.labelZh) : type;
}
