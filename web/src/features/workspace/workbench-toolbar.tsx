import { Activity, ArrowLeftRight, Bot, FileDiff, FolderTree, Maximize2, MoreHorizontal, PanelRight, SquareTerminal } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "../../shared/ui/button/button";
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import { modKeyLabel } from "../../shared/mod-key";
import { useI18n } from "../i18n/use-i18n";
import { OPEN_WORKSPACE_PANEL_EVENT } from "./workspace-panel-options";
import type { PaneTab } from "./workspace-tab";
import { requestWorkbenchCommand } from "./workbench-shortcuts";
import "./workbench-toolbar.css";

type WorkbenchToolbarProps = {
  workspaceOpen: boolean;
  terminalOpen: boolean;
  activePanel: PaneTab | null;
  overview?: ReactNode;
  onToggleWorkspace: () => void;
  onSwap: () => void;
  onMaximize: () => void;
};

/**
 * 将文件、变更审阅和布局操作组合为会话标题栏工具组。
 * @param props 当前面板状态、运行概览和布局回调
 * @returns 紧凑工作台工具栏
 */
export function WorkbenchToolbar({ workspaceOpen, terminalOpen, activePanel, overview, onToggleWorkspace, onSwap, onMaximize }: WorkbenchToolbarProps) {
  const { t } = useI18n();
  const modifier = modKeyLabel();
  /** 通知工作区打开指定面板，文件入口同时展开目录树。 */
  const openPanel = (tab: PaneTab) => window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab, revealFileTree: tab === "files" } }));
  return (
    <div className="workbench-toolbar" role="toolbar" aria-label={t("Workbench actions", "工作台操作")}>
      <Button variant="ghost" size="small" className="workbench-tool" aria-label={t("Browse files", "浏览文件")} title={t(`Browse files (${modifier}+Shift+E)`, `浏览文件 (${modifier}+Shift+E)`)} aria-pressed={workspaceOpen && activePanel === "files"} onClick={() => openPanel("files")}>
        <FolderTree size={15} /><span className="workbench-tool-label hidden xl:inline">{t("Files", "文件")}</span>
      </Button>
      <Button variant="ghost" size="small" className="workbench-tool workbench-review" aria-label={t("Review changes", "审阅变更")} title={t(`Review changes (${modifier}+Shift+G)`, `审阅变更 (${modifier}+Shift+G)`)} aria-pressed={workspaceOpen && activePanel === "diff"} onClick={() => openPanel("diff")}>
        <FileDiff size={15} /><span className="workbench-tool-label">{t("Review", "审阅")}</span>
      </Button>
      <Button variant="ghost" size="icon" className="workbench-terminal-tool hidden md:inline-flex" aria-label={t("Toggle bottom terminal", "切换底部终端")} title={t(`Terminal (${modifier}+J)`, `终端 (${modifier}+J)`)} aria-pressed={terminalOpen} onClick={() => requestWorkbenchCommand("toggle-terminal")}><SquareTerminal size={15} /></Button>
      {overview}
      <ActionMenu label={t("Workbench menu", "工作台菜单")} trigger={<MoreHorizontal size={17} />} items={[
        { id: "terminal", label: t("Toggle terminal", "切换终端"), icon: <SquareTerminal size={15} />, shortcut: `${modifier}+J`, onSelect: () => requestWorkbenchCommand("toggle-terminal") },
        { id: "tasks", label: t("Background tasks", "后台任务"), icon: <Activity size={15} />, onSelect: () => openPanel("tasks") },
        { id: "subagents", label: t("Subagents", "子智能体"), icon: <Bot size={15} />, onSelect: () => openPanel("subagents") },
        { id: "sidebar", label: workspaceOpen ? t("Hide side panel", "隐藏侧面板") : t("Show side panel", "显示侧面板"), icon: <PanelRight size={15} />, separator: true, onSelect: onToggleWorkspace },
        { id: "swap", label: t("Swap chat and workspace", "交换对话与工作区"), icon: <ArrowLeftRight size={15} />, disabled: !workspaceOpen, onSelect: onSwap },
        { id: "maximize", label: t("Maximize workspace", "最大化工作区"), icon: <Maximize2 size={15} />, disabled: !workspaceOpen, onSelect: onMaximize }
      ]} />
    </div>
  );
}
