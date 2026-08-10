import { Activity, Bot, FileCode2, GitCompareArrows, MessageSquarePlus, Server, SquareTerminal } from "lucide-react";
import type { PaneTab } from "./workspace-tab";

/**
 * 侧栏可打开的功能标识。
 *
 * 除面板类型外还包含 "ssh"：它最终打开的仍是终端面板，
 * 但创建过程要先选主机，因此在菜单里单列一项，
 * 否则 SSH 能力藏在终端面板内部，入口不可见。
 */
export type WorkspacePanelAction = PaneTab | "ssh";

export type WorkspacePanelOption = {
  type: WorkspacePanelAction;
  labelEn: string;
  labelZh: string;
  icon: typeof FileCode2;
};

/**
 * 工作区可打开面板的统一配置。
 *
 * 空侧栏引导页和工作区标签栏新增菜单共用此列表，避免多处维护同一组面板选项。
 */
export const WORKSPACE_PANEL_OPTIONS: WorkspacePanelOption[] = [
  { type: "files", labelEn: "Editor", labelZh: "编辑器", icon: FileCode2 },
  { type: "diff", labelEn: "Git", labelZh: "Git", icon: GitCompareArrows },
  { type: "terminal", labelEn: "Terminal", labelZh: "终端", icon: SquareTerminal },
  { type: "ssh", labelEn: "SSH terminal", labelZh: "SSH 终端", icon: Server },
  { type: "tasks", labelEn: "Background tasks", labelZh: "后台任务", icon: Activity },
  { type: "subagents", labelEn: "Subagents", labelZh: "子智能体", icon: Bot },
  { type: "side-chat", labelEn: "Side conversation", labelZh: "旁路对话", icon: MessageSquarePlus }
];

/** 用户可从空侧栏或新增菜单主动打开的功能。 */
export const ACTIVE_WORKSPACE_PANEL_OPTIONS = WORKSPACE_PANEL_OPTIONS;

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
