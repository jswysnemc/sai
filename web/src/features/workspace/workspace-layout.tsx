import { PanelRightOpen } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import type { CSSProperties } from "react";
import { useEffect, useReducer, useState } from "react";
import { api } from "../../api/client";
import { ChatPage } from "../chat/chat-page";
import { SessionSidebar } from "../sessions/session-sidebar";
import { SessionSidebarResizeHandle } from "../sessions/session-sidebar-resize-handle";
import { useSessionSidebarLayout } from "../sessions/use-session-sidebar-layout";
import { WorkspacePane } from "./workspace-pane";
import { WorkspaceResizeHandle } from "./workspace-resize-handle";
import { useWorkspaceLayout } from "./use-workspace-layout";
import { workspaceRelativePath } from "./workspace-path-utils";
import type { PaneTab } from "./workspace-tab";
import { useTerminalManager } from "../terminal/use-terminal-manager";
import {
  initialMobileWorkbenchState,
  MOBILE_SIDEBAR_TOGGLE_EVENT,
  MOBILE_WORKBENCH_MEDIA_QUERY,
  reduceMobileWorkbenchState
} from "./mobile-workbench-state";
import { OPEN_WORKSPACE_PANEL_EVENT } from "./workspace-panel-options";
import {
  OPEN_WORKSPACE_DIFF_EVENT,
  OPEN_WORKSPACE_SIDEBAR_EVENT,
  type WorkspacePassiveDiff
} from "./workspace-passive-diff";
import "./workspace-pane.css";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceLayoutProps = {
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
};

/**
 * 组合会话栏、聊天区和可调整的右侧工作区。
 *
 * @param props 文件选择状态与回调
 * @returns 编程工作区布局
 */
export function WorkspaceLayout({ selectedFile, onSelectFile, onClearFile }: WorkspaceLayoutProps) {
  const { t } = useI18n();
  const layout = useWorkspaceLayout();
  const terminalManager = useTerminalManager();
  const sessionSidebar = useSessionSidebarLayout();
  const [paneTab, setPaneTab] = useState<PaneTab | null>(null);
  const [passiveDiff, setPassiveDiff] = useState<WorkspacePassiveDiff | null>(null);
  const [fileTreeRequestId, setFileTreeRequestId] = useState(0);
  const [mobileLayout, dispatchMobileLayout] = useReducer(reduceMobileWorkbenchState, initialMobileWorkbenchState);
  const [isMobile, setIsMobile] = useState(() => window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches);
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const activeWorkspace = workspaces.data?.workspaces.find((workspace) => workspace.id === workspaces.data.active_id);
  const style = {
    "--session-sidebar-width": `${sessionSidebar.width}px`,
    "--workspace-panel-width": `${layout.workspaceWidth}px`
  } as CSSProperties;
  const classes = [
    "coding-layout",
    layout.workspaceOpen ? "workspace-open" : "workspace-closed",
    layout.chatOpen ? "chat-open" : "chat-closed",
    layout.workspaceMaximized ? "workspace-maximized" : "",
    layout.swapped ? "layout-swapped" : "",
    sessionSidebar.collapsed ? "sidebar-collapsed" : "sidebar-expanded",
    mobileLayout.sidebarOpen ? "mobile-sidebar-open" : "mobile-sidebar-closed",
    `mobile-pane-${mobileLayout.pane}`
  ].filter(Boolean).join(" ");

  useEffect(() => {
    const media = window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY);
    /**
     * 同步当前视口是否使用移动端工作台。
     *
     * @param event 媒体查询变化事件
     */
    const handleMobileChange = (event: MediaQueryListEvent) => setIsMobile(event.matches);
    setIsMobile(media.matches);
    media.addEventListener("change", handleMobileChange);
    return () => media.removeEventListener("change", handleMobileChange);
  }, []);

  useEffect(() => {
    /** 响应顶部 Logo 发出的移动端会话侧栏切换事件。 */
    const handleSidebarToggle = () => {
      if (!window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) return;
      if (!mobileLayout.sidebarOpen) sessionSidebar.expand();
      dispatchMobileLayout({ type: "toggle-sidebar" });
    };
    window.addEventListener(MOBILE_SIDEBAR_TOGGLE_EVENT, handleSidebarToggle);
    return () => window.removeEventListener(MOBILE_SIDEBAR_TOGGLE_EVENT, handleSidebarToggle);
  }, [mobileLayout.sidebarOpen, sessionSidebar.expand]);

  useEffect(() => {
    /** 响应消息区发出的"在编辑器中打开文件"事件。 */
    const handleOpenFile = (event: Event) => {
      const detail = (event as CustomEvent<{ path?: string; reveal?: boolean }>).detail;
      const path = detail?.path;
      if (!path) return;
      const relativePath = workspaceRelativePath(path, activeWorkspace?.path ?? "");
      onSelectFile(relativePath);
      layout.openWorkspace();
      setPaneTab("files");
      setPassiveDiff(null);
      if (detail.reveal) {
        setFileTreeRequestId((current) => current + 1);
      }
      if (window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) {
        dispatchMobileLayout({ type: "show-pane", pane: "workspace" });
      }
    };
    window.addEventListener("sai:open-file", handleOpenFile);
    return () => window.removeEventListener("sai:open-file", handleOpenFile);
  }, [activeWorkspace?.path, onSelectFile, layout.openWorkspace]);

  useEffect(() => {
    /** 响应聊天区发出的终端/后台任务/面板入口，打开右侧同级面板。 */
    const openPanel = (tab: PaneTab) => {
      layout.openWorkspace();
      setPaneTab(tab);
      if (tab !== "diff") setPassiveDiff(null);
      if (window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) {
        dispatchMobileLayout({ type: "show-pane", pane: tab === "terminal" ? "terminal" : "workspace" });
      }
    };
    const handleToggleTerminal = () => openPanel("terminal");
    const handleOpenTasks = () => openPanel("tasks");
    const handleOpenSubagents = () => openPanel("subagents");
    const handleOpenSidebar = () => {
      layout.openWorkspace();
      setPaneTab(null);
      setPassiveDiff(null);
      if (window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) {
        dispatchMobileLayout({ type: "show-pane", pane: "workspace" });
      }
    };
    const handleOpenDiff = (event: Event) => {
      const detail = (event as CustomEvent<WorkspacePassiveDiff>).detail;
      if (!detail?.path) return;
      layout.openWorkspace();
      setPassiveDiff(detail);
      setPaneTab("diff");
      if (window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) {
        dispatchMobileLayout({ type: "show-pane", pane: "workspace" });
      }
    };
    /** 响应移动端聊天头部 `+` 菜单发出的面板打开请求。 */
    const handleOpenPanel = (event: Event) => {
      const detail = (event as CustomEvent<{ tab?: PaneTab; revealFileTree?: boolean }>).detail;
      const tab = detail?.tab;
      if (tab) openPanel(tab);
      if (detail?.revealFileTree) setFileTreeRequestId((current) => current + 1);
    };
    window.addEventListener("sai:toggle-terminal", handleToggleTerminal);
    window.addEventListener("sai:open-tasks", handleOpenTasks);
    window.addEventListener("sai:open-subagents", handleOpenSubagents);
    window.addEventListener(OPEN_WORKSPACE_SIDEBAR_EVENT, handleOpenSidebar);
    window.addEventListener(OPEN_WORKSPACE_DIFF_EVENT, handleOpenDiff);
    window.addEventListener(OPEN_WORKSPACE_PANEL_EVENT, handleOpenPanel);
    return () => {
      window.removeEventListener("sai:toggle-terminal", handleToggleTerminal);
      window.removeEventListener("sai:open-tasks", handleOpenTasks);
      window.removeEventListener("sai:open-subagents", handleOpenSubagents);
      window.removeEventListener(OPEN_WORKSPACE_SIDEBAR_EVENT, handleOpenSidebar);
      window.removeEventListener(OPEN_WORKSPACE_DIFF_EVENT, handleOpenDiff);
      window.removeEventListener(OPEN_WORKSPACE_PANEL_EVENT, handleOpenPanel);
    };
  }, [layout.openWorkspace]);

  /** 关闭工作区，并在移动端回到聊天面板。 */
  const closeWorkspace = () => {
    layout.closeWorkspace();
    setPaneTab(null);
    setPassiveDiff(null);
    dispatchMobileLayout({ type: "show-pane", pane: "chat" });
  };

  /**
   * 从收起态直接打开空侧栏。
   *
   * 返回:
   * - 无返回值
   */
  const openEmptyWorkspace = () => {
    setPaneTab(null);
    setPassiveDiff(null);
    layout.openWorkspace();
    if (window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) {
      dispatchMobileLayout({ type: "show-pane", pane: "workspace" });
    }
  };

  return (
    <div className={classes} style={style}>
      {mobileLayout.sidebarOpen && (
        <button type="button" className="mobile-sidebar-scrim" onClick={() => dispatchMobileLayout({ type: "close-sidebar" })} aria-label={t("Close session sidebar", "关闭会话侧栏")} />
      )}
      <aside className="coding-sidebar" aria-hidden={isMobile && !mobileLayout.sidebarOpen} inert={isMobile && !mobileLayout.sidebarOpen}>
        <SessionSidebar
          collapsed={sessionSidebar.collapsed}
          onToggleCollapsed={sessionSidebar.toggleCollapsed}
          onNavigate={() => dispatchMobileLayout({ type: "close-sidebar" })}
        />
        {!sessionSidebar.collapsed && <SessionSidebarResizeHandle width={sessionSidebar.width} onResize={sessionSidebar.resize} />}
      </aside>
      <div className="workbench-main">
        {layout.chatOpen && !layout.workspaceMaximized && <section className="coding-chat"><ChatPage /></section>}
        {layout.workspaceOpen && layout.chatOpen && !layout.workspaceMaximized && <WorkspaceResizeHandle swapped={layout.swapped} onResize={layout.resizeWorkspace} />}
        {layout.workspaceOpen && (
          <aside className="coding-workspace">
            <WorkspacePane
              selectedFile={selectedFile}
              activeType={paneTab}
              passiveDiff={passiveDiff}
              fileTreeRequestId={fileTreeRequestId}
              onFileTreeRequestHandled={() => setFileTreeRequestId(0)}
              maximized={layout.workspaceMaximized}
              onActiveTypeChange={setPaneTab}
              onSelectFile={onSelectFile}
              onClearFile={onClearFile}
              onToggleMaximized={layout.toggleWorkspaceMaximized}
              onCollapse={closeWorkspace}
              terminalManager={terminalManager}
            />
          </aside>
        )}
        {!layout.workspaceOpen && (
          <div className="workspace-reopen-anchor">
            <button
              type="button"
              className="workspace-reopen"
              onClick={openEmptyWorkspace}
              title={t("Open workspace panel", "打开工作区")}
              aria-label={t("Open workspace panel", "打开工作区")}
            >
              <PanelRightOpen size={16} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
