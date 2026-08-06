import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Cable, CalendarClock, CheckSquare2, ChevronDown, ChevronRight, FolderOpen, MoreHorizontal, PanelLeftClose, PanelLeftOpen, Pencil, Plus, Search, Settings, Square, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { localizeApiMessage, toDisplayError } from "../../api/api-error";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { SkeletonList } from "../../shared/ui/skeleton/skeleton";
import { SaiLogo } from "../../shared/ui/sai-logo";
import { switchWithTerminalConfirm } from "../workspaces/workspace-switcher";
import { ServerDirectoryDialog } from "../workspaces/server-directory-dialog";
import { FileTree } from "../workspace/file-tree";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { useSessionTree } from "./use-session-tree";
import { LocaleSwitcher } from "../i18n/locale-switcher";
import { useI18n } from "../i18n/use-i18n";
import { formatRelativeTime } from "../../shared/format-relative-time";
import { initializeNewSessionPreferences } from "./new-session-preferences";
import { SessionWorkspaceIcon } from "./session-workspace-icon";
import { SessionSidebarActions } from "./session-sidebar-actions";
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
 * 渲染会话列表、新建入口和批量管理模式。
 *
 * @param props 折叠状态和切换回调
 * @returns 会话侧栏
 */
export function SessionSidebar({ collapsed, onToggleCollapsed, onNavigate, selectedFile, onSelectFile, onClearFile }: SessionSidebarProps) {
  const { locale, t } = useI18n();
  const queryClient = useQueryClient();
  const confirm = useConfirm();
  const navigate = useNavigate();
  const location = useLocation();
  const [menu, setMenu] = useState<string | null>(null);
  const [workspaceMenu, setWorkspaceMenu] = useState<string | null>(null);
  const [appMenuOpen, setAppMenuOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [sessionSearch, setSessionSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [navigationError, setNavigationError] = useState<Error | null>(null);
  const [sidebarView, setSidebarView] = useState<SidebarView>("workspaces");
  // 相对时间每分钟刷新一次
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNowTick(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const appMenuRef = useRef<HTMLDivElement | null>(null);
  const { tree, expanded, runningSessions, toggleWorkspace } = useSessionTree();
  const activeWorkspace = tree.data?.find((workspace) => workspace.active);
  const sessions = activeWorkspace?.sessions ?? [];

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
   * 【会话】【缓存刷新】刷新会话列表和全部消息缓存。
   *
   * @returns 全部相关缓存刷新完成后返回
   */
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    await queryClient.invalidateQueries({ queryKey: ["session-tree"] });
    await queryClient.invalidateQueries({ queryKey: ["messages"] });
    await queryClient.invalidateQueries({ queryKey: ["timeline"] });
  };

  /**
   * 切换工作区和会话，跨工作区时完成切换后重新载入工作台。
   *
   * @param workspaceId 目标工作区 ID
   * @param sessionId 目标会话 ID
   * @param workspaceActive 目标工作区是否已经激活
   * @param sessionActive 目标会话是否已经激活
   * @returns 切换流程完成后返回
   */
  const openSession = async (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => {
    setNavigationError(null);
    try {
      if (sessionActive) {
        onNavigate?.();
        return;
      }
      if (!workspaceActive) {
        const switched = await switchWithTerminalConfirm(workspaceId, confirm, t);
        if (!switched) return;
      }
      await api.sessions.switch(sessionId);
      if (!workspaceActive) window.location.reload();
      else await refresh();
      onNavigate?.();
    } catch (cause) {
      setNavigationError(toDisplayError(cause, "Failed to open session", "打开会话失败"));
    }
  };

  /** 切换到指定工作区；工作区视图不强制打开某个会话。 */
  const openWorkspace = async (workspaceId: string, workspaceActive: boolean) => {
    if (workspaceActive) return;
    setNavigationError(null);
    try {
      const switched = await switchWithTerminalConfirm(workspaceId, confirm, t);
      if (switched) window.location.reload();
    } catch (cause) {
      setNavigationError(toDisplayError(cause, "Failed to open workspace", "打开工作区失败"));
    }
  };

  /**
   * 【会话】【新会话默认值】创建会话并在列表刷新前写入专属模型与思考偏好。
   *
   * @param workspaceId 可选目标工作区 ID
   * @returns 新建会话
   */
  const createSession = async (workspaceId?: string) => {
    const response = await queryClient.ensureQueryData({
      queryKey: ["config"],
      queryFn: api.config.load
    });
    const engine = response.config.agent?.engine ?? "native";
    // 1. 【会话】【新会话默认值】外部内核先读取当前能力，失败时按内核默认值创建
    const status = engine === "native"
      ? undefined
      : await queryClient.fetchQuery({
          queryKey: ["engine-status"],
          queryFn: api.config.engineStatus
        }).catch(() => undefined);
    // 2. 【会话】【新会话默认值】服务端创建成功后立即建立会话专属偏好
    const session = await api.sessions.create(undefined, workspaceId);
    initializeNewSessionPreferences(session.id, response.config, status);
    return session;
  };

  const create = useMutation({
    mutationFn: createSession,
    onSuccess: async (session, workspaceId) => {
      // 1. 先刷新会话树，使新会话立即出现在目标工作区
      await refresh();
      const targetWorkspaceId = workspaceId ?? activeWorkspace?.workspace_id;
      if (!targetWorkspaceId) return;
      // 2. 非活动工作区先切换工作区，再激活刚创建的会话
      await openSession(targetWorkspaceId, session.id, workspaceId === undefined, session.active);
    }
  });
  const remove = useMutation({ mutationFn: api.sessions.remove, onSuccess: refresh });
  const rename = useMutation({
    mutationFn: ({ id, title }: { id: string; title: string }) => api.sessions.rename(id, title),
    onSuccess: async () => {
      setRenaming(null);
      await refresh();
    }
  });
  const removeMany = useMutation({
    mutationFn: api.sessions.removeMany,
    onSuccess: async () => {
      setSelected(new Set());
      setSelecting(false);
      setConfirming(false);
      await refresh();
    }
  });
  const removeWorkspace = useMutation({
    mutationFn: api.workspaces.remove,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      await queryClient.invalidateQueries({ queryKey: ["session-tree"] });
    }
  });

  /**
   * 切换指定会话的选中状态。
   *
   * @param id 会话 ID
   */
  const toggleSelected = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setConfirming(false);
  };

  /**
   * 全选或取消全选当前工作区会话。
   *
   * @returns 无返回值
   */
  const toggleAll = () => {
    const ids = sessions.map((session) => session.id);
    const allSelected = ids.length > 0 && ids.every((id) => selected.has(id));
    setSelected(allSelected ? new Set() : new Set(ids));
    setConfirming(false);
  };

  /**
   * 删除所选会话；先弹出危险确认，避免误触。
   *
   * @returns 无返回值
   */
  const requestBulkDelete = async () => {
    if (selected.size === 0 || removeMany.isPending) return;
    setConfirming(true);
    const count = selected.size;
    const accepted = await confirm({
      title: t("Delete sessions", "删除会话"),
      description: t(
        `Delete ${count} selected session(s)? This cannot be undone.`,
        `删除已选的 ${count} 个会话？此操作不可撤销。`
      ),
      confirmLabel: t("Delete", "删除"),
      cancelLabel: t("Cancel", "取消"),
      danger: true
    });
    setConfirming(false);
    if (!accepted) return;
    removeMany.mutate(Array.from(selected));
  };

  /** 退出选择模式并清理临时状态。 */
  const closeSelection = () => {
    setSelecting(false);
    setSelected(new Set());
    setConfirming(false);
  };

  /**
   * 进入多选模式；不默认勾选任何会话。
   *
   * @returns 无返回值
   */
  const enterSelectionMode = () => {
    setSelecting(true);
    setSelected(new Set());
    setConfirming(false);
  };

  /**
   * 登记服务端目录并切换到对应工作区。
   *
   * @param path 服务端目录路径
   */
  const openDirectory = async (path: string) => {
    const workspace = await api.workspaces.add(path);
    const switched = await switchWithTerminalConfirm(workspace.id, confirm, t);
    if (switched) window.location.reload();
  };

  /**
   * 执行统一搜索面板选中的应用操作。
   *
   * @param action 搜索结果对应操作
   * @returns 无返回值
   */
  const runSearchAction = (action: GlobalSearchAction) => {
    if (action === "new-session") create.mutate(undefined);
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

  /** 确认后关闭非活动工作区。 */
  const closeWorkspace = async (workspaceId: string, workspaceName: string, workspaceActive: boolean) => {
    setNavigationError(null);
    try {
      const accepted = await confirm({
        title: t("Close workspace", "关闭工作区"),
        description: t(`Close “${workspaceName}” from the list? Workspace files will not be deleted.`, `从列表中关闭“${workspaceName}”？工作区文件不会被删除。`),
        confirmLabel: t("Close", "关闭")
      });
      if (!accepted) return;
      if (workspaceActive) {
        const fallback = tree.data?.find((workspace) => workspace.workspace_id !== workspaceId);
        if (!fallback) return;
        const switched = await switchWithTerminalConfirm(fallback.workspace_id, confirm, t);
        if (!switched) return;
        await api.workspaces.remove(workspaceId);
        window.location.reload();
        return;
      }
      removeWorkspace.mutate(workspaceId);
    } catch (cause) {
      setNavigationError(toDisplayError(cause, "Failed to close workspace", "关闭工作区失败"));
    }
  };

  /**
   * 进入指定会话的重命名编辑态。
   *
   * @param id 会话 ID
   * @param title 当前标题
   */
  const startRename = (id: string, title: string) => {
    setRenaming(id);
    setRenameDraft(title);
    setMenu(null);
  };

  /** 提交重命名，标题为空或未变化时直接退出编辑态。 */
  const submitRename = () => {
    if (!renaming) return;
    const title = renameDraft.trim();
    const current = sessions.find((session) => session.id === renaming);
    if (!title || title === current?.title) {
      setRenaming(null);
      return;
    }
    rename.mutate({ id: renaming, title });
  };

  const error = navigationError ?? tree.error ?? create.error ?? remove.error ?? removeMany.error ?? rename.error ?? removeWorkspace.error;
  const appMenuActive = location.pathname.startsWith("/settings")
    || location.pathname.startsWith("/gateways")
    || location.pathname.startsWith("/cron-jobs");

  const query = sessionSearch.trim().toLowerCase();
  const searchableWorkspaces = sidebarView === "sessions"
    ? activeWorkspace ? [activeWorkspace] : []
    : tree.data ?? [];
  const visibleWorkspaces = searchableWorkspaces.filter((workspace) => {
    if (!query) return true;
    if (workspace.workspace_name.toLowerCase().includes(query)) return true;
    if (localizeApiMessage(workspace.workspace_name, locale).toLowerCase().includes(query)) return true;
    return workspace.sessions.some(
      (session) => session.title.toLowerCase().includes(query) || session.id.toLowerCase().includes(query)
    );
  });

  if (collapsed) {
    return (
      <div className="session-sidebar collapsed">
        <button type="button" className="sidebar-rail-button brand-rail" onClick={onToggleCollapsed} aria-label={t("Expand session sidebar", "展开会话侧栏")} title={t("Expand session sidebar", "展开会话侧栏")}>
          <SaiLogo size={18} />
        </button>
        <button type="button" className="sidebar-rail-button" onClick={onToggleCollapsed} aria-label={t("Expand session sidebar", "展开会话侧栏")} title={t("Expand session sidebar", "展开会话侧栏")}>
          <PanelLeftOpen size={17} />
        </button>
        <button type="button" className="sidebar-rail-button" onClick={() => setBrowserOpen(true)} aria-label={t("Open server directory", "打开服务端目录")} title={t("Open server directory", "打开服务端目录")}>
          <FolderOpen size={17} />
        </button>
        <button type="button" className="sidebar-rail-button" onClick={() => create.mutate(undefined)} disabled={create.isPending} aria-label={t("New session", "新建会话")} title={t("New session", "新建会话")}>
          <Plus size={17} />
        </button>
        <div className="sidebar-app-menu collapsed-app-menu" ref={appMenuRef}>
          <button
            type="button"
            className={`sidebar-rail-button${appMenuOpen || appMenuActive ? " active" : ""}`}
            onClick={() => setAppMenuOpen((value) => !value)}
            aria-label={t("Settings menu", "设置菜单")}
            title={t("Settings menu", "设置菜单")}
            aria-expanded={appMenuOpen}
          >
            <Settings size={17} strokeWidth={1.8} />
          </button>
          <LocaleSwitcher compact />
          {appMenuOpen && (
            <div className="sidebar-app-popover rail">
              <button type="button" onClick={() => { setAppMenuOpen(false); setBrowserOpen(true); }}>
                <FolderOpen size={14} /><span>{t("Open server directory", "打开服务端目录")}</span>
              </button>
              <button type="button" onClick={() => { setAppMenuOpen(false); navigate("/settings"); onNavigate?.(); }}>
                <Settings size={14} /><span>{t("Settings", "设置")}</span>
              </button>
              <button type="button" onClick={() => { setAppMenuOpen(false); navigate("/gateways"); onNavigate?.(); }}>
                <Cable size={14} /><span>{t("Gateways", "网关")}</span>
              </button>
              <button type="button" onClick={() => { setAppMenuOpen(false); navigate("/cron-jobs"); onNavigate?.(); }}>
                <CalendarClock size={14} /><span>{t("Scheduled tasks", "定时任务")}</span>
              </button>
            </div>
          )}
        </div>
        <ServerDirectoryDialog open={browserOpen} onClose={() => setBrowserOpen(false)} onSelect={openDirectory} />
        <GlobalSearchDialog
          open={searchOpen}
          workspaces={tree.data ?? []}
          onClose={() => setSearchOpen(false)}
          onAction={runSearchAction}
          onOpenSession={(workspaceId, sessionId) => {
            const workspace = tree.data?.find((item) => item.workspace_id === workspaceId);
            const session = workspace?.sessions.find((item) => item.id === sessionId);
            if (!workspace || !session) return;
            void openSession(workspaceId, sessionId, workspace.active, session.active);
          }}
        />
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
          onNewSession={() => create.mutate(undefined)}
          onSearch={() => setSearchOpen(true)}
          onScheduledTasks={() => navigate("/cron-jobs")}
          onSkills={() => navigate("/settings/skills")}
        />
      )}
      {sidebarView !== "files" && (
        <label className="session-search">
          <Search size={14} />
          <input
            value={sidebarView === "workspaces" ? "" : sessionSearch}
            readOnly={sidebarView === "workspaces"}
            onFocus={() => { if (sidebarView === "workspaces") setSearchOpen(true); }}
            onClick={() => { if (sidebarView === "workspaces") setSearchOpen(true); }}
            onChange={(event) => setSessionSearch(event.target.value)}
            placeholder={sidebarView === "workspaces" ? t("Search", "搜索") : t("Search sessions", "搜索会话")}
            aria-label={sidebarView === "workspaces" ? t("Search actions, tasks or files", "搜索操作、任务或文件") : t("Search sessions", "搜索会话")}
            spellCheck={false}
          />
          {sidebarView !== "workspaces" && sessionSearch && (
            <button type="button" className="session-search-clear" onClick={() => setSessionSearch("")} aria-label={t("Clear search", "清空搜索")}>
              <X size={13} />
            </button>
          )}
        </label>
      )}
      {sidebarView === "files" ? (
        <div className="sidebar-file-tree-view">
          <FileTree
            selectedFile={selectedFile}
            onSelectFile={onSelectFile}
            onClearFile={onClearFile}
            onClose={() => setSidebarView("sessions")}
          />
        </div>
      ) : <div className={`session-list sidebar-${sidebarView}-view`}>
        {tree.isLoading && (
          <div className="sidebar-skeleton">
            <SkeletonList items={6} label={t("Loading sessions", "读取会话")} />
          </div>
        )}
        {!tree.isLoading && query && visibleWorkspaces.length === 0 && (
          <div className="sidebar-state">{t(`No sessions match “${sessionSearch.trim()}”`, `没有匹配“${sessionSearch.trim()}”的会话`)}</div>
        )}
        {visibleWorkspaces.map((workspace) => {
          const workspaceName = localizeApiMessage(workspace.workspace_name, locale);
          const sessions = query
            ? workspace.sessions.filter((session) =>
                session.title.toLowerCase().includes(query)
                || session.id.toLowerCase().includes(query)
              )
            : workspace.sessions;
          const workspaceExpanded = sidebarView === "sessions" ? true : false;
          const workspaceRunning = sessions.some((session) => runningSessions.has(`${workspace.workspace_id}:${session.id}`));
          const canSelect = workspace.active && sessions.length > 0;
          const canClose = (tree.data?.length ?? 0) > 1;
          return <div className="session-workspace" key={workspace.workspace_id}>
            <div className={`${workspace.active ? "workspace-tree-row active" : "workspace-tree-row"}${workspace.active && selecting ? " selecting" : ""}`}>
              <button type="button" className="workspace-tree-main" onClick={() => {
                if (sidebarView === "workspaces") {
                  void openWorkspace(workspace.workspace_id, workspace.active);
                } else if (!query) {
                  toggleWorkspace(workspace.workspace_id);
                }
              }} aria-expanded={workspaceExpanded}>
                {workspaceExpanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                <SessionWorkspaceIcon isGitRepository={workspace.is_git_repository} size={14} />
                <span className="workspace-summary">
                  <strong>{workspaceName}</strong>
                  {workspaceRunning && <ActiveAgentIndicator />}
                  <small>{t(`${sessions.length} sessions`, `${sessions.length} 个会话`)}</small>
                </span>
              </button>
              <span className="workspace-tree-actions">
                {!selecting && <button type="button" className="workspace-create-session" onClick={() => create.mutate(workspace.active ? undefined : workspace.workspace_id)} disabled={create.isPending} aria-label={t(`Create a session in ${workspaceName}`, `在 ${workspaceName} 新建会话`)} title={t("New session", "新建会话")}><Plus size={14} /></button>}
                {!(workspace.active && selecting) && (canSelect || canClose) && (
                  <button type="button" onClick={() => { setMenu(null); setWorkspaceMenu((value) => value === workspace.workspace_id ? null : workspace.workspace_id); }} aria-label={t(`Manage workspace ${workspaceName}`, `管理工作区 ${workspaceName}`)} title={t("Manage workspace", "管理工作区")}><MoreHorizontal size={14} /></button>
                )}
                {workspace.active && selecting && (
                  <button type="button" onClick={closeSelection} aria-label={t("Exit selection", "退出选择")} title={t("Exit selection", "退出选择")}>
                    <X size={14} />
                  </button>
                )}
              </span>
              {workspaceMenu === workspace.workspace_id && (
                <div className="session-menu workspace-menu-popover" ref={menuRef}>
                  {canSelect && <button type="button" onClick={() => { setWorkspaceMenu(null); enterSelectionMode(); }}><CheckSquare2 size={14} /> {t("Select sessions", "多选会话")}</button>}
                  {canClose && <button type="button" className="danger" onClick={() => { setWorkspaceMenu(null); void closeWorkspace(workspace.workspace_id, workspaceName, workspace.active); }}><X size={14} /> {t("Close workspace", "关闭工作区")}</button>}
                </div>
              )}
            </div>
            {workspace.active && selecting && (
              <div className="workspace-selection-bar" role="toolbar" aria-label={t("Session selection", "会话多选")}>
                <button
                  type="button"
                  className="selection-action"
                  onClick={toggleAll}
                  disabled={sessions.length === 0}
                  aria-label={selected.size === sessions.length && sessions.length > 0 ? t("Clear selection", "取消全选") : t("Select all", "全选")}
                  title={selected.size === sessions.length && sessions.length > 0 ? t("Clear selection", "取消全选") : t("Select all", "全选")}
                >
                  <CheckSquare2 size={13} />
                  <span>{selected.size === sessions.length && sessions.length > 0 ? t("Clear", "取消全选") : t("Select all", "全选")}</span>
                </button>
                <span className="workspace-selection-count">{t(`${selected.size} selected`, `已选 ${selected.size}`)}</span>
                <button
                  type="button"
                  className={confirming || removeMany.isPending ? "selection-delete confirming" : "selection-delete"}
                  onClick={() => void requestBulkDelete()}
                  disabled={selected.size === 0 || removeMany.isPending}
                  aria-label={t("Delete selected sessions", "删除所选会话")}
                  title={t("Delete selected sessions", "删除所选会话")}
                >
                  <Trash2 size={13} />
                  <span>{removeMany.isPending ? t("Deleting", "删除中") : t("Delete", "删除")}</span>
                </button>
              </div>
            )}
            {sidebarView === "sessions" && workspaceExpanded && <div className="workspace-session-children">{sessions.map((session) => {
          const checked = selected.has(session.id);
          const running = runningSessions.has(`${workspace.workspace_id}:${session.id}`);
          return (
            <div className={`${session.active ? "session-row active" : "session-row"}${checked ? " selected" : ""}${running ? " running" : ""}`} key={session.id}>
              {selecting && workspace.active && (
                <button type="button" className="session-check" onClick={() => toggleSelected(session.id)} aria-label={t(`Select ${session.title}`, `选择 ${session.title}`)}>
                  {checked ? <CheckSquare2 size={15} /> : <Square size={15} />}
                </button>
              )}
              {!selecting && renaming === session.id ? (
                <div className="session-rename">
                  <input
                    autoFocus
                    value={renameDraft}
                    disabled={rename.isPending}
                    onChange={(event) => setRenameDraft(event.target.value)}
                    onKeyDown={(event) => {
                      // 1. 回车提交重命名
                      if (event.key === "Enter") submitRename();
                      // 2. Esc 取消编辑
                      if (event.key === "Escape") setRenaming(null);
                    }}
                    onBlur={() => setRenaming(null)}
                    aria-label={t(`Rename ${session.title}`, `重命名 ${session.title}`)}
                  />
                </div>
              ) : (
                <button type="button" className="session-main" onClick={() => {
                  if (selecting && workspace.active) {
                    toggleSelected(session.id);
                    return;
                  }
                  void openSession(workspace.workspace_id, session.id, workspace.active, session.active);
                }}>
                  <span className="session-summary">
                    <strong>{session.title}</strong>
                    {running && <ActiveAgentIndicator />}
                    <small title={new Date(session.updated_at).toLocaleString(locale)}>{formatRelativeTime(session.updated_at, locale, nowTick)}</small>
                  </span>
                </button>
              )}
              {!selecting && workspace.active && renaming !== session.id && <button type="button" className="session-more" aria-label={t(`Manage ${session.title}`, `管理 ${session.title}`)} onClick={() => setMenu((value) => value === session.id ? null : session.id)}><MoreHorizontal size={15} /></button>}
              {!selecting && menu === session.id && (
                <div className="session-menu" ref={menuRef}>
                  <button type="button" onClick={() => startRename(session.id, session.title)}><Pencil size={14} /> {t("Rename", "重命名")}</button>
                  <button type="button" className="danger" onClick={() => { void (async () => {
                    setMenu(null);
                    const accepted = await confirm({
                      title: t("Delete session", "删除会话"),
                      description: t(`Delete “${session.title}”? This cannot be undone.`, `删除“${session.title}”？此操作不可撤销。`),
                      confirmLabel: t("Delete", "删除"),
                      cancelLabel: t("Cancel", "取消"),
                      danger: true
                    });
                    if (accepted) remove.mutate(session.id);
                  })(); }}><Trash2 size={14} /> {t("Delete", "删除")}</button>
                </div>
              )}
            </div>
          );
        })}</div>}
          </div>;
        })}
      </div>}
      {error && <p className="sidebar-error">{error.message}</p>}
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
          <div className="sidebar-app-popover">
            <button type="button" onClick={() => { setAppMenuOpen(false); setBrowserOpen(true); }}>
              <FolderOpen size={14} /><span>{t("Open server directory", "打开服务端目录")}</span>
            </button>
            <NavLink to="/settings" onClick={() => { setAppMenuOpen(false); onNavigate?.(); }} className={({ isActive }) => isActive ? "active" : ""}>
              <Settings size={14} /><span>{t("Settings", "设置")}</span>
            </NavLink>
            <NavLink to="/gateways" onClick={() => { setAppMenuOpen(false); onNavigate?.(); }} className={({ isActive }) => isActive ? "active" : ""}>
              <Cable size={14} /><span>{t("Gateways", "网关")}</span>
            </NavLink>
            <NavLink to="/cron-jobs" onClick={() => { setAppMenuOpen(false); onNavigate?.(); }} className={({ isActive }) => isActive ? "active" : ""}>
              <CalendarClock size={14} /><span>{t("Scheduled tasks", "定时任务")}</span>
            </NavLink>
          </div>
        )}
      </div>
      <ServerDirectoryDialog open={browserOpen} onClose={() => setBrowserOpen(false)} onSelect={openDirectory} />
      <GlobalSearchDialog
        open={searchOpen}
        workspaces={tree.data ?? []}
        onClose={() => setSearchOpen(false)}
        onAction={runSearchAction}
        onOpenSession={(workspaceId, sessionId) => {
          const workspace = tree.data?.find((item) => item.workspace_id === workspaceId);
          const session = workspace?.sessions.find((item) => item.id === sessionId);
          if (!workspace || !session) return;
          void openSession(workspaceId, sessionId, workspace.active, session.active);
        }}
      />
    </div>
  );
}
