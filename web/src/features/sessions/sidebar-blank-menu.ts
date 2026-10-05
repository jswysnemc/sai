import type { ReactNode } from "react";
import type { ActionMenuItem } from "../../shared/ui/menu/action-menu";
import type { SidebarBrowseMode } from "./sidebar-browse";

type Translate = (en: string, zh: string) => string;

/** 侧栏空白处菜单可用的动作与当前状态。 */
export type SidebarBlankActions = {
  mode: SidebarBrowseMode;
  historyOpen: boolean;
  hasSessions: boolean;
  createPending: boolean;
  onNewSession: () => void;
  onSearch: () => void;
  onSelectSessions: () => void;
  onToggleHistory: () => void;
  onAddWorkspace: () => void;
  onRefresh: () => void;
  icons: { create: ReactNode; search: ReactNode; select: ReactNode; history: ReactNode; workspace: ReactNode; refresh: ReactNode };
};

/**
 * 【会话侧栏】【空白右键】按当前浏览模式生成菜单项。
 *
 * 会话页：新建会话、搜索、批量选择、展开或收起历史、刷新。
 * 工作区页：新建会话、加入工作区、搜索、刷新。
 *
 * @param actions 动作、状态与图标
 * @param t 双语文本
 * @returns 菜单项
 */
export function sidebarBlankMenuItems(actions: SidebarBlankActions, t: Translate): ActionMenuItem[] {
  const create = { id: "new-session", label: t("New Session", "新建会话"), icon: actions.icons.create, disabled: actions.createPending, onSelect: actions.onNewSession };
  const search = { id: "search", label: t("Search…", "搜索…"), icon: actions.icons.search, onSelect: actions.onSearch };
  const refresh = { id: "refresh", label: t("Refresh", "刷新"), icon: actions.icons.refresh, separator: true, onSelect: actions.onRefresh };
  if (actions.mode === "workspaces") {
    return [
      create,
      { id: "add-workspace", label: t("Add Workspace…", "加入工作区…"), icon: actions.icons.workspace, onSelect: actions.onAddWorkspace },
      search,
      refresh
    ];
  }
  return [
    create,
    search,
    { id: "select", label: t("Select Sessions", "批量选择会话"), icon: actions.icons.select, separator: true, disabled: !actions.hasSessions, onSelect: actions.onSelectSessions },
    { id: "history", label: actions.historyOpen ? t("Collapse History", "收起历史会话") : t("Expand History", "展开历史会话"), icon: actions.icons.history, onSelect: actions.onToggleHistory },
    refresh
  ];
}

/**
 * 判断右键是否落在侧栏空白处：会话行、按钮、输入框等交互元素都有自己的菜单或原生行为。
 *
 * @param target 事件目标
 * @returns 落在空白处时返回 true
 */
export function isSidebarBlankTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return !target.closest("button, a, input, textarea, [role='menu'], [role='treeitem'], [role='option'], li, .sidebar-file-tree-view");
}
