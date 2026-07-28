import { FolderGit2, GitBranch, PanelLeft, Plus } from "lucide-react";
import { useRef, useState } from "react";
import type { Workspace } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { useOutsidePointerDown } from "../../shared/hooks/use-outside-pointer-down";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { MOBILE_SIDEBAR_TOGGLE_EVENT } from "../workspace/mobile-workbench-state";
import { OPEN_WORKSPACE_PANEL_EVENT, WORKSPACE_PANEL_OPTIONS } from "../workspace/workspace-panel-options";
import type { PaneTab } from "../workspace/workspace-tab";
import "./chat-session-header.css";

type ChatSessionHeaderProps = {
  title: string;
  workspace?: Pick<Workspace, "name" | "path"> | null;
  branch?: string;
};

/**
 * 渲染会话标题、项目上下文和移动端工作区操作。
 *
 * @param props 会话标题、当前工作区和 Git 分支
 * @returns 聊天页面头部
 */
export function ChatSessionHeader({ title, workspace, branch }: ChatSessionHeaderProps) {
  const { locale, t } = useI18n();
  const [panelMenuOpen, setPanelMenuOpen] = useState(false);
  const panelMenuRef = useRef<HTMLDivElement>(null);
  const workspaceName = workspace ? localizeApiMessage(workspace.name, locale) : "";
  const hasContext = Boolean(workspace?.path || branch);
  useOutsidePointerDown(panelMenuRef, () => setPanelMenuOpen(false), panelMenuOpen);

  /**
   * 打开指定工作区面板，移动端会切换到全屏面板视图。
   *
   * @param tab 目标工作区面板
  * @returns 无返回值
  */
  const openWorkspacePanel = (tab: PaneTab) => {
    // 【Web 对话】【面板导航】1. 先关闭移动端面板菜单
    setPanelMenuOpen(false);
    // 【Web 对话】【面板导航】2. 再通知工作台切换到目标面板
    window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab } }));
  };

  return (
    <header className="chat-header">
      <Button
        className="chat-header-menu"
        onClick={() => window.dispatchEvent(new Event(MOBILE_SIDEBAR_TOGGLE_EVENT))}
        aria-label={t("Open session sidebar", "打开会话侧栏")}
        title={t("Open session sidebar", "打开会话侧栏")}
      >
        <PanelLeft size={16} aria-hidden />
      </Button>
      <div className="chat-header-main">
        <h1 title={title}>{title}</h1>
        {hasContext && (
          <div className="chat-header-context" aria-label={t("Project context", "项目上下文")}>
            {workspace?.path && (
              <span
                className="chat-header-context-item chat-header-workspace"
                title={workspace.path}
                aria-label={t(
                  `Project ${workspaceName}, path ${workspace.path}`,
                  `项目 ${workspaceName}，路径 ${workspace.path}`
                )}
              >
                <FolderGit2 size={12} aria-hidden />
                <span className="chat-header-path">{workspace.path}</span>
                <span className="chat-header-workspace-name">{workspaceName}</span>
              </span>
            )}
            {branch && (
              <span
                className="chat-header-context-item chat-header-branch"
                title={t(`Git branch: ${branch}`, `Git 分支：${branch}`)}
                aria-label={t(`Git branch: ${branch}`, `Git 分支：${branch}`)}
              >
                <GitBranch size={12} aria-hidden />
                <span>{branch}</span>
              </span>
            )}
          </div>
        )}
      </div>
      <div className="chat-header-panel" ref={panelMenuRef}>
        <Button
          className="chat-header-plus"
          onClick={() => setPanelMenuOpen((value) => !value)}
          aria-expanded={panelMenuOpen}
          aria-haspopup="menu"
          aria-label={t("Open workspace panel", "打开工作区面板")}
          title={t("Open workspace panel", "打开工作区面板")}
        >
          <Plus size={16} aria-hidden />
        </Button>
        {panelMenuOpen && (
          <div className="chat-header-panel-menu" role="menu" aria-label={t("Choose panel", "选择面板")}>
            {WORKSPACE_PANEL_OPTIONS.map((item) => {
              const Icon = item.icon;
              return (
                <Button role="menuitem" key={item.type} onClick={() => openWorkspacePanel(item.type)}>
                  <Icon size={14} aria-hidden />
                  <span>{t(item.labelEn, item.labelZh)}</span>
                </Button>
              );
            })}
          </div>
        )}
      </div>
    </header>
  );
}
