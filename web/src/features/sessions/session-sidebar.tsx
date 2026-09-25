import { PanelLeftClose } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { SaiLogo } from "../../shared/ui/sai-logo";
import { ServerDirectoryDialog } from "../workspaces/server-directory-dialog";
import { OPEN_SIDEBAR_FILE_TREE_EVENT } from "./sidebar-file-tree-cover";
import { useSessionTree } from "./use-session-tree";
import { useSessionActions } from "./use-session-actions";
import { useSessionSelection } from "./use-session-selection";
import { LocaleSwitcher } from "../i18n/locale-switcher";
import { useI18n } from "../i18n/use-i18n";
import { SessionSidebarActions } from "./session-sidebar-actions";
import { Button } from "../../shared/ui/button/button";
import { SESSION_SCOPE_KEY, type SessionScope } from "./session-scope-control";
import { useRunningSessions } from "./session-running-state";
import { WORKBENCH_COMMAND_EVENT, type WorkbenchCommand } from "../workspace/workbench-shortcuts";
import { FileTree } from "../workspace/file-tree";
import { readBrowseMode, writeBrowseMode, type SidebarBrowseMode } from "./sidebar-browse";
import { SidebarTaskBoard } from "./sidebar-task-board";
import { SidebarAppMenu } from "./sidebar-app-menu";
import { SidebarCollapsedRail } from "./sidebar-collapsed-rail";
import { GlobalSearchDialog, type GlobalSearchAction } from "../search/global-search-dialog";
import { OPEN_WORKSPACE_PANEL_EVENT } from "../workspace/workspace-panel-options";
import "./session-sidebar.css";
import "./session-sidebar-workspaces.css";
import "./sidebar-purpose-section.css";

type SessionSidebarProps = {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onNavigate?: () => void;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
  selectedSessionId?: string;
  onSessionSelected?: (workspaceId: string, sessionId: string) => void;
};

/**
 * 渲染会话侧栏：项目、会话与文件树三个可折叠分区。
 *
 * 数据操作在 useSessionActions、多选在 useSessionSelection、
 * 各分区各自成组件，本组件只保管分区展开、菜单开合这类布局状态。
 *
 * @param props 折叠状态和切换回调
 * @returns 会话侧栏
 */
export function SessionSidebar({ collapsed, onToggleCollapsed, onNavigate, selectedFile, onSelectFile, onClearFile, selectedSessionId, onSessionSelected }: SessionSidebarProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const navigate = useNavigate();
  const [menu, setMenu] = useState<string | null>(null);
  const [workspaceMenu, setWorkspaceMenu] = useState<string | null>(null);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [browse, setBrowse] = useState<SidebarBrowseMode>(readBrowseMode);
  const [sessionScope] = useState<SessionScope>(() => localStorage.getItem(SESSION_SCOPE_KEY) === "recent" || localStorage.getItem(SESSION_SCOPE_KEY) === "all" ? "recent" : "current");
  const runningSessions = useRunningSessions();
  useEffect(() => { localStorage.setItem(SESSION_SCOPE_KEY, sessionScope); }, [sessionScope]);

  // 相对时间每分钟刷新一次
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNowTick(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const { tree } = useSessionTree(selectedSessionId);

  const actions = useSessionActions({
    confirm,
    t,
    tree: () => tree.data,
    onNavigate,
    onSessionSelected
  });
  const selection = useSessionSelection({
    confirm,
    t,
    removeMany: (ids) => actions.removeMany.mutateAsync(ids)
  });

  useEffect(() => {
    const openFileTree = () => {
      writeBrowseMode("files");
      setBrowse("files");
    };
    window.addEventListener(OPEN_SIDEBAR_FILE_TREE_EVENT, openFileTree);
    return () => window.removeEventListener(OPEN_SIDEBAR_FILE_TREE_EVENT, openFileTree);
  }, []);

  useEffect(() => {
    /** 【会话导航】【快捷操作】处理命令菜单与新建会话请求。 */
    const handleCommand = (event: Event) => {
      const command = (event as CustomEvent<WorkbenchCommand>).detail;
      if (command === "search") setSearchOpen(true);
      if (command === "new-session" && !actions.create.isPending) actions.create.mutate(undefined);
    };
    window.addEventListener(WORKBENCH_COMMAND_EVENT, handleCommand);
    return () => window.removeEventListener(WORKBENCH_COMMAND_EVENT, handleCommand);
  }, [actions.create]);

  // 1. 监听整页 pointerdown，点击菜单外任意位置时关闭会话或工作区管理菜单
  useEffect(() => {
    if (!menu && !workspaceMenu) return;
    /**
     * 处理菜单外部点击并关闭菜单。
     *
     * @param event 指针事件
     */
    const onPointerDown = (event: PointerEvent) => {
      if (menuRef.current && event.target instanceof Node && menuRef.current.contains(event.target)) return;
      setMenu(null);
      setWorkspaceMenu(null);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [menu, workspaceMenu]);

  /**
   * 执行统一搜索面板选中的应用操作。
   *
   * @param action 搜索结果对应操作
   * @returns 无返回值
   */
  const runSearchAction = (action: GlobalSearchAction) => {
    if (action === "new-session") actions.create.mutate(undefined);
    else if (action === "open-workspace") setBrowserOpen(true);
    else if (action === "settings") navigate("/settings");
    else if (action === "scheduled-tasks") navigate("/cron-jobs");
    else if (action === "toggle-terminal") window.dispatchEvent(new Event("sai:toggle-terminal"));
    else if (action === "open-tasks") window.dispatchEvent(new Event("sai:open-tasks"));
    else if (action === "open-subagents") window.dispatchEvent(new Event("sai:open-subagents"));
    else if (action === "open-git") window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab: "diff" } }));
    else if (action === "open-files") {
      writeBrowseMode("files");
      setBrowse("files");
      window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab: "files" } }));
    }
    else if (action === "toggle-sidebar") onToggleCollapsed();
    onNavigate?.();
  };

  /** 打开搜索面板选中的会话。 */
  const openSearchSession = (workspaceId: string, sessionId: string) => {
    const workspace = tree.data?.find((item) => item.workspace_id === workspaceId);
    const session = workspace?.sessions.find((item) => item.id === sessionId);
    if (!workspace || !session) return;
    void actions.openSession(workspaceId, sessionId, workspace.active, session.active);
  };

  const dialogs = (
    <>
      <ServerDirectoryDialog open={browserOpen} onClose={() => setBrowserOpen(false)} onSelect={actions.openDirectory} />
      <GlobalSearchDialog
        open={searchOpen}
        workspaces={tree.data ?? []}
        onClose={() => setSearchOpen(false)}
        onAction={runSearchAction}
        onOpenSession={openSearchSession}
        onOpenFile={onSelectFile}
      />
    </>
  );

  if (collapsed) {
    return (
      <div className="session-sidebar collapsed">
        <SidebarCollapsedRail
          onExpand={onToggleCollapsed}
          onNewSession={() => actions.create.mutate(undefined)}
          newSessionPending={actions.create.isPending}
          onOpenDirectory={() => setBrowserOpen(true)}
          onSearch={() => setSearchOpen(true)}
          onAfterNavigate={onNavigate}
        />
        {dialogs}
      </div>
    );
  }

  const workspaces = tree.data ?? [];

  return (
    <div className="session-sidebar">
      <div className="sidebar-heading">
        <Button variant="ghost" className="sidebar-brand" onClick={onToggleCollapsed} aria-label="Sai" title="Sai">
          <SaiLogo size={48} trim />
        </Button>
        <div className="sidebar-heading-actions">
          <Button variant="ghost" size="icon" className="icon-button" aria-label={t("Collapse session sidebar", "折叠会话侧栏")} title={t("Collapse session sidebar", "折叠会话侧栏")} onClick={onToggleCollapsed}>
            <PanelLeftClose size={16} />
          </Button>
        </div>
      </div>
      <SessionSidebarActions
        onNewSession={() => actions.create.mutate(undefined)}
        onSearch={() => setSearchOpen(true)}
        onScheduledTasks={() => { navigate("/cron-jobs"); onNavigate?.(); }}
        onSkills={() => { navigate("/settings/skills"); onNavigate?.(); }}
        createPending={actions.create.isPending}
      />
      <SidebarTaskBoard
        workspaces={workspaces}
        loading={tree.isLoading}
        runningSessions={runningSessions}
        now={nowTick}
        sessionScope={sessionScope}
        selection={selection}
        menuRef={menuRef}
        menu={menu}
        workspaceMenu={workspaceMenu}
        createPending={actions.create.isPending}
        onToggleMenu={setMenu}
        onToggleWorkspaceMenu={setWorkspaceMenu}
        onOpenSession={(workspaceId, sessionId, workspaceActive, sessionActive) => void actions.openSession(workspaceId, sessionId, workspaceActive, sessionActive)}
        onRename={async (id, title) => { await actions.rename.mutateAsync({ id, title }); }}
        onDelete={(id, title) => void actions.removeWithConfirm(id, title)}
        onCreateSession={(workspaceId, active) => actions.create.mutate(active ? undefined : workspaceId)}
        onCloseWorkspace={(workspaceId, name, active) => void actions.closeWorkspace(workspaceId, name, active)}
        onAddWorkspace={() => setBrowserOpen(true)}
        browse={browse}
        onBrowse={(mode) => {
          writeBrowseMode(mode);
          setBrowse(mode);
        }}
        selectedSessionId={selectedSessionId}
        files={
          <FileTree
            selectedFile={selectedFile}
            onSelectFile={onSelectFile}
            onClearFile={onClearFile}
            showHeading={false}
            workspaceKey={workspaces.find((workspace) => workspace.active)?.workspace_id}
            workspaceLabel={workspaces.find((workspace) => workspace.active)?.workspace_name}
            searchPlaceholder={t("Search files...", "搜索文件...")}
          />
        }
      />
      {(actions.error ?? tree.error) && <p className="sidebar-error">{(actions.error ?? tree.error)?.message}</p>}
      <div className="sidebar-footer">
        <div className="sidebar-footer-actions">
          <SidebarAppMenu onOpenDirectory={() => setBrowserOpen(true)} onAfterNavigate={onNavigate} />
          <LocaleSwitcher />
        </div>
      </div>
      {dialogs}
    </div>
  );
}
