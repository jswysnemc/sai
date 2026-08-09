import { PanelLeftClose, Settings } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
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
  const location = useLocation();
  const [menu, setMenu] = useState<string | null>(null);
  const [workspaceMenu, setWorkspaceMenu] = useState<string | null>(null);
  const [appMenuOpen, setAppMenuOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [sidebarView, setSidebarView] = useState<SidebarView>("sessions");
  // 相对时间每分钟刷新一次
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNowTick(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const appMenuRef = useRef<HTMLDivElement | null>(null);
  const { tree, runningSessions } = useSessionTree();
  const activeWorkspace = tree.data?.find((workspace) => workspace.active);

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
    /** 处理全局搜索快捷键。 */
    const handleSearchShortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "k") return;
      event.preventDefault();
      setSearchOpen(true);
    };
    window.addEventListener("keydown", handleSearchShortcut);
    return () => window.removeEventListener("keydown", handleSearchShortcut);
  }, []);

  // 1. 监听整页 pointerdown，点击菜单外任意位置时关闭会话或工作区管理菜单
  useEffect(() => {
    if (!menu && !workspaceMenu && !appMenuOpen) return;
    /**
     * 处理菜单外部点击并关闭菜单。
     *
     * @param event 指针事件
     */
    const onPointerDown = (event: PointerEvent) => {
      if (menuRef.current && event.target instanceof Node && menuRef.current.contains(event.target)) return;
      if (appMenuRef.current && event.target instanceof Node && appMenuRef.current.contains(event.target)) return;
      setMenu(null);
      setWorkspaceMenu(null);
      setAppMenuOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [appMenuOpen, menu, workspaceMenu]);

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
    else if (action === "open-files") window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab: "files" } }));
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

  const appMenuActive = location.pathname.startsWith("/settings")
    || location.pathname.startsWith("/gateways")
    || location.pathname.startsWith("/cron-jobs");

  const dialogs = (
    <>
      <ServerDirectoryDialog open={browserOpen} onClose={() => setBrowserOpen(false)} onSelect={actions.openDirectory} />
      <GlobalSearchDialog
        open={searchOpen}
        workspaces={tree.data ?? []}
        onClose={() => setSearchOpen(false)}
        onAction={runSearchAction}
        onOpenSession={openSearchSession}
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
          appMenuOpen={appMenuOpen}
          onToggleAppMenu={() => setAppMenuOpen((value) => !value)}
          onCloseAppMenu={() => setAppMenuOpen(false)}
          appMenuActive={appMenuActive}
          appMenuRef={appMenuRef}
          onAfterNavigate={onNavigate}
        />
        {dialogs}
      </div>
    );
  }

  return (
    <div className="session-sidebar">
      <div className="sidebar-heading">
        <button type="button" className="sidebar-brand" onClick={onToggleCollapsed} aria-label="Sai" title="Sai">
          <SaiLogo size={20} />
          <span>Sai</span>
        </button>
        <div className="sidebar-heading-actions">
          <button type="button" className="icon-button" aria-label={t("Collapse session sidebar", "折叠会话侧栏")} title={t("Collapse session sidebar", "折叠会话侧栏")} onClick={onToggleCollapsed}>
            <PanelLeftClose size={16} />
          </button>
        </div>
      </div>
      <div className="sidebar-view-switcher" role="tablist" aria-label={t("Sidebar view", "侧栏视图")}>
        {([
          ["sessions", t("Sessions", "会话")],
          ["workspaces", t("Workspaces", "工作区")],
          ["files", t("Files", "文件树")]
        ] as const).map(([view, label]) => (
          <button
            key={view}
            type="button"
            role="tab"
            aria-selected={sidebarView === view}
            className={sidebarView === view ? "active" : ""}
            onClick={() => setSidebarView(view)}
          >
            {label}
          </button>
        ))}
      </div>
      {sidebarView === "sessions" && (
        <SessionSidebarActions
          onNewSession={() => actions.create.mutate(undefined)}
          onSearch={() => setSearchOpen(true)}
        />
      )}
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
        activeWorkspace ? (
          <SessionListView
            workspace={activeWorkspace}
            runningSessions={runningSessions}
            selection={selection}
            now={nowTick}
            menuRef={menuRef}
            menu={menu}
            onToggleMenu={setMenu}
            onOpenSession={(sessionId, sessionActive) =>
              void actions.openSession(activeWorkspace.workspace_id, sessionId, true, sessionActive)}
            onRename={async (id, title) => {
              await actions.rename.mutateAsync({ id, title });
            }}
            onDelete={(id, title) => void actions.removeWithConfirm(id, title)}
          />
        ) : (
          <div className="sidebar-state">{t("No sessions in this view", "当前视图没有内容")}</div>
        )
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
          <div className="sidebar-state">{t("No sessions in this view", "当前视图没有内容")}</div>
        )
      )}
      {(actions.error ?? tree.error) && <p className="sidebar-error">{(actions.error ?? tree.error)?.message}</p>}
      <div className="sidebar-footer" ref={appMenuRef}>
        <div className="sidebar-footer-actions">
          <button
            type="button"
            className={`sidebar-settings-link${appMenuOpen || appMenuActive ? " active" : ""}`}
            onClick={() => setAppMenuOpen((value) => !value)}
            aria-expanded={appMenuOpen}
          >
            <Settings size={15} strokeWidth={1.8} /><span>{t("Settings", "设置")}</span>
          </button>
          <LocaleSwitcher />
        </div>
        {appMenuOpen && (
          <SidebarAppMenu
            onClose={() => setAppMenuOpen(false)}
            onOpenDirectory={() => setBrowserOpen(true)}
            onAfterNavigate={onNavigate}
          />
        )}
      </div>
      {dialogs}
    </div>
  );
}
