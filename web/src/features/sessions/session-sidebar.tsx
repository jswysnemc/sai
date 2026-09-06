import { FolderPlus, PanelLeftClose } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { SkeletonList } from "../../shared/ui/skeleton/skeleton";
import { SaiLogo } from "../../shared/ui/sai-logo";
import { ServerDirectoryDialog } from "../workspaces/server-directory-dialog";
import { FileTree } from "../workspace/file-tree";
import { useSessionTree } from "./use-session-tree";
import { useSessionActions } from "./use-session-actions";
import { useSessionSelection } from "./use-session-selection";
import { LocaleSwitcher } from "../i18n/locale-switcher";
import { useI18n } from "../i18n/use-i18n";
import { SessionSidebarActions } from "./session-sidebar-actions";
import { Button } from "../../shared/ui/button/button";
import { SegmentedControl } from "../../shared/ui/segmented-control";
import { SidebarProjectGroup } from "./sidebar-project-group";
import { SessionScopeControl, SESSION_SCOPE_KEY, type SessionScope } from "./session-scope-control";
import { useRunningSessions } from "./session-running-state";
import { WORKBENCH_COMMAND_EVENT, type WorkbenchCommand } from "../workspace/workbench-shortcuts";
import { SessionListView } from "./session-list-view";
import { WorkspaceListView } from "./workspace-list-view";
import { SidebarAppMenu } from "./sidebar-app-menu";
import { SidebarCollapsedRail } from "./sidebar-collapsed-rail";
import { GlobalSearchDialog, type GlobalSearchAction } from "../search/global-search-dialog";
import { OPEN_WORKSPACE_PANEL_EVENT } from "../workspace/workspace-panel-options";
import "./session-sidebar.css";
import "./session-sidebar-workspaces.css";

type SessionSidebarProps = {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onNavigate?: () => void;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
};

type SidebarView = "sessions" | "workspaces" | "files";

/**
 * 渲染会话侧栏：会话、工作区与文件树三个视图的壳。
 *
 * 数据操作在 useSessionActions、多选在 useSessionSelection、
 * 各视图各自成组件，本组件只保管视图切换、菜单开合这类布局状态。
 *
 * @param props 折叠状态和切换回调
 * @returns 会话侧栏
 */
export function SessionSidebar({ collapsed, onToggleCollapsed, onNavigate, selectedFile, onSelectFile, onClearFile }: SessionSidebarProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const navigate = useNavigate();
  const [menu, setMenu] = useState<string | null>(null);
  const [workspaceMenu, setWorkspaceMenu] = useState<string | null>(null);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [sidebarView, setSidebarView] = useState<SidebarView>("sessions");
  const [sessionScope, setSessionScope] = useState<SessionScope>(() => localStorage.getItem(SESSION_SCOPE_KEY) === "all" ? "all" : "current");
  const runningSessions = useRunningSessions();
  useEffect(() => { localStorage.setItem(SESSION_SCOPE_KEY, sessionScope); }, [sessionScope]);
  // 相对时间每分钟刷新一次
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNowTick(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const { tree } = useSessionTree();

  const actions = useSessionActions({
    confirm,
    t,
    tree: () => tree.data,
    onNavigate
  });
  const selection = useSessionSelection({
    confirm,
    t,
    removeMany: (ids) => actions.removeMany.mutateAsync(ids)
  });

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
    else if (action === "open-files") window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab: "files", revealFileTree: true } }));
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
      <SegmentedControl className="sidebar-view-switcher" value={sidebarView} onChange={setSidebarView} ariaLabel={t("Sidebar view", "侧栏视图")} options={[
        { value: "sessions", label: t("Sessions", "会话") },
        { value: "workspaces", label: t("Workspaces", "工作区") },
        { value: "files", label: t("Files", "文件") }
      ]} />
      {sidebarView === "sessions" && <SessionScopeControl value={sessionScope} onChange={(scope) => { setSessionScope(scope); selection.exitSelection(); }} />}
      {sidebarView === "files" && (
        <div className="sidebar-file-tree-view">
          <FileTree
            selectedFile={selectedFile}
            onSelectFile={onSelectFile}
            onClearFile={onClearFile}
            onClose={() => setSidebarView("sessions")}
          />
        </div>
      )}
      {sidebarView !== "files" && tree.isLoading && (
        <div className="sidebar-skeleton">
          <SkeletonList items={6} label={t("Loading sessions", "读取会话")} />
        </div>
      )}
      {sidebarView === "sessions" && !tree.isLoading && (
        <div className={`sidebar-projects${sessionScope === "current" ? " is-current-workspace" : ""}`}>
          {(tree.data ?? []).filter((workspace) => sessionScope === "all" || workspace.active).map((workspace) => {
            const sessions = <SessionListView
              key={workspace.workspace_id}
              workspace={workspace}
              runningSessions={runningSessions}
              showWorkspaceHeader={sessionScope === "current"}
              selection={selection}
              now={nowTick}
              menuRef={menuRef}
              menu={menu}
              onToggleMenu={setMenu}
              onOpenSession={(sessionId, sessionActive) => void actions.openSession(workspace.workspace_id, sessionId, workspace.active, sessionActive)}
              onRename={async (id, title) => { await actions.rename.mutateAsync({ id, title }); }}
              onDelete={(id, title) => void actions.removeWithConfirm(id, title)}
            />;
            return sessionScope === "current" ? sessions : <SidebarProjectGroup
              key={workspace.workspace_id}
              workspace={workspace}
              pending={actions.create.isPending}
              canClose={!workspace.active || (tree.data?.length ?? 0) > 1}
              onCreate={() => actions.create.mutate(workspace.active ? undefined : workspace.workspace_id)}
              onOpen={() => void actions.openWorkspace(workspace.workspace_id, workspace.active)}
              onClose={() => void actions.closeWorkspace(workspace.workspace_id, workspace.workspace_name, workspace.active)}
            >{sessions}</SidebarProjectGroup>;
          })}
          {!tree.data?.length && <div className="sidebar-state">{t("Add a project to get started.", "加入项目后即可开始。")}</div>}
        </div>
      )}
      {sidebarView === "workspaces" && (
        <div className="sidebar-session-actions" role="toolbar" aria-label={t("Workspace actions", "工作区操作")}>
          <Button variant="ghost" onClick={() => setBrowserOpen(true)}>
            <FolderPlus size={14} /><span>{t("Add workspace", "加入工作区")}</span>
          </Button>
        </div>
      )}
      {sidebarView === "workspaces" && !tree.isLoading && (
        (tree.data?.length ?? 0) > 0 ? (
          <WorkspaceListView
            workspaces={tree.data ?? []}
            runningSessions={runningSessions}
            menuRef={menuRef}
            menu={workspaceMenu}
            onToggleMenu={setWorkspaceMenu}
            onOpenWorkspace={(workspaceId, active) => void actions.openWorkspace(workspaceId, active)}
            onCreateSession={(workspaceId, active) => actions.create.mutate(active ? undefined : workspaceId)}
            createPending={actions.create.isPending}
            onCloseWorkspace={(workspaceId, name, active) => void actions.closeWorkspace(workspaceId, name, active)}
          />
        ) : (
          <div className="sidebar-state">{t("No workspaces yet — add one above", "还没有工作区，点击上方「加入工作区」开始")}</div>
        )
      )}
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
