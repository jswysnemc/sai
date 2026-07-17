import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Cable, CalendarClock, CheckSquare2, ChevronDown, ChevronRight, FolderGit2, FolderOpen, MoreHorizontal, PanelLeftClose, PanelLeftOpen, Pencil, Plus, RefreshCw, Search, Settings, Square, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { SaiLogo } from "../../shared/ui/sai-logo";
import { switchWithTerminalConfirm } from "../workspaces/workspace-switcher";
import { ServerDirectoryDialog } from "../workspaces/server-directory-dialog";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { useSessionTree } from "./use-session-tree";
import { useI18n } from "../i18n/use-i18n";
import "./session-sidebar.css";

type SessionSidebarProps = {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onNavigate?: () => void;
};

/**
 * 渲染会话列表、新建入口和批量管理模式。
 *
 * @param props 折叠状态和切换回调
 * @returns 会话侧栏
 */
export function SessionSidebar({ collapsed, onToggleCollapsed, onNavigate }: SessionSidebarProps) {
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
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [navigationError, setNavigationError] = useState<Error | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const appMenuRef = useRef<HTMLDivElement | null>(null);
  const { tree, expanded, runningSessions, toggleWorkspace } = useSessionTree();
  const activeWorkspace = tree.data?.find((workspace) => workspace.active);
  const sessions = activeWorkspace?.sessions ?? [];

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

  /** 刷新会话列表和全部消息缓存。 */
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    await queryClient.invalidateQueries({ queryKey: ["session-tree"] });
    await queryClient.invalidateQueries({ queryKey: ["messages"] });
    await queryClient.invalidateQueries({ queryKey: ["timeline"] });
  };

  const create = useMutation({ mutationFn: (workspaceId?: string) => api.sessions.create(undefined, workspaceId), onSuccess: refresh });
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

  /** 切换全部可删除会话的选中状态。 */
  const toggleAll = () => {
    const ids = sessions.filter((session) => session.id !== "default").map((session) => session.id);
    setSelected(selected.size === ids.length ? new Set() : new Set(ids));
    setConfirming(false);
  };

  /** 执行两阶段批量删除确认。 */
  const requestBulkDelete = () => {
    if (selected.size === 0) return;
    if (!confirming) {
      setConfirming(true);
      return;
    }
    removeMany.mutate(Array.from(selected));
  };

  /** 退出选择模式并清理临时状态。 */
  const closeSelection = () => {
    setSelecting(false);
    setSelected(new Set());
    setConfirming(false);
  };

  /** 从当前工作区进入全选模式。 */
  const selectWorkspaceSessions = () => {
    setSelecting(true);
    toggleAll();
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
      setNavigationError(cause instanceof Error ? cause : new Error(String(cause)));
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

  const manageableCount = sessions.filter((session) => session.id !== "default").length;
  const error = navigationError ?? tree.error ?? create.error ?? remove.error ?? removeMany.error ?? rename.error ?? removeWorkspace.error;
  const appMenuActive = location.pathname.startsWith("/settings")
    || location.pathname.startsWith("/gateways")
    || location.pathname.startsWith("/cron-jobs");

  /** 切换工作区和会话，跨工作区时完成切换后重新载入工作台。 */
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
      setNavigationError(cause instanceof Error ? cause : new Error(String(cause)));
    }
  };

  const query = sessionSearch.trim().toLowerCase();
  const visibleWorkspaces = (tree.data ?? []).filter((workspace) => {
    if (!query) return true;
    if (workspace.workspace_name.toLowerCase().includes(query)) return true;
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
            aria-label={t("Application menu", "应用菜单")}
            title={t("Application menu", "应用菜单")}
            aria-expanded={appMenuOpen}
          >
            <Settings size={17} strokeWidth={1.8} />
          </button>
          {appMenuOpen && (
            <div className="sidebar-app-popover rail">
              <button type="button" onClick={() => { setAppMenuOpen(false); setBrowserOpen(true); }}>
                <FolderOpen size={14} /><span>{t("Open server directory", "打开服务端目录")}</span>
              </button>
              <button type="button" onClick={() => { setAppMenuOpen(false); navigate("/settings"); onNavigate?.(); }}>
                <Settings size={14} /><span>{t("Settings", "配置")}</span>
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
      <label className="session-search">
        <Search size={14} />
        <input
          value={sessionSearch}
          onChange={(event) => setSessionSearch(event.target.value)}
          placeholder={t("Search sessions", "搜索会话")}
          aria-label={t("Search sessions", "搜索会话")}
          spellCheck={false}
        />
        {sessionSearch && (
          <button type="button" className="session-search-clear" onClick={() => setSessionSearch("")} aria-label={t("Clear search", "清空搜索")}>
            <X size={13} />
          </button>
        )}
      </label>
      <div className="session-list">
        {tree.isLoading && <div className="sidebar-state"><RefreshCw size={15} className="spin" /> {t("Loading sessions", "读取会话")}</div>}
        {!tree.isLoading && query && visibleWorkspaces.length === 0 && (
          <div className="sidebar-state">{t(`No sessions match “${sessionSearch.trim()}”`, `没有匹配“${sessionSearch.trim()}”的会话`)}</div>
        )}
        {visibleWorkspaces.map((workspace) => {
          const sessions = query
            ? workspace.sessions.filter((session) =>
                session.title.toLowerCase().includes(query)
                || session.id.toLowerCase().includes(query)
              )
            : workspace.sessions;
          const workspaceExpanded = query ? true : expanded.has(workspace.workspace_id);
          const workspaceRunning = sessions.some((session) => runningSessions.has(`${workspace.workspace_id}:${session.id}`));
          const canSelect = workspace.active && manageableCount > 0;
          const canClose = (tree.data?.length ?? 0) > 1;
          return <div className="session-workspace" key={workspace.workspace_id}>
            <div className={`${workspace.active ? "workspace-tree-row active" : "workspace-tree-row"}${workspace.active && selecting ? " selecting" : ""}`}>
              <button type="button" className="workspace-tree-main" onClick={() => !query && toggleWorkspace(workspace.workspace_id)} aria-expanded={workspaceExpanded}>
                {workspaceExpanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                <FolderGit2 size={14} />
                <span><strong>{workspace.workspace_name}</strong><small>{t(`${sessions.length} sessions`, `${sessions.length} 个会话`)}</small></span>
                {workspaceRunning && <ActiveAgentIndicator />}
              </button>
              <span className="workspace-tree-actions">
                {!selecting && <button type="button" className="workspace-create-session" onClick={() => create.mutate(workspace.active ? undefined : workspace.workspace_id)} disabled={create.isPending} aria-label={t(`Create a session in ${workspace.workspace_name}`, `在 ${workspace.workspace_name} 新建会话`)} title={t("New session", "新建会话")}><Plus size={14} /></button>}
                {workspace.active && selecting && <span className="workspace-selection-count">{t(`${selected.size} selected`, `已选择 ${selected.size} 项`)}</span>}
                {workspace.active && selecting && <button type="button" className={confirming ? "danger confirming" : "danger"} onClick={requestBulkDelete} disabled={selected.size === 0 || removeMany.isPending} aria-label={confirming ? t(`Confirm deletion of ${selected.size} items`, `确认删除 ${selected.size} 项`) : t("Delete selected sessions", "删除所选会话")} title={confirming ? t(`Confirm deletion of ${selected.size} items`, `确认删除 ${selected.size} 项`) : t("Delete selected sessions", "删除所选会话")}><Trash2 size={14} /></button>}
                {workspace.active && selecting && <button type="button" onClick={closeSelection} aria-label={t("Exit selection", "退出选择")} title={t("Exit selection", "退出选择")}><X size={14} /></button>}
                {!(workspace.active && selecting) && (canSelect || canClose) && (
                  <button type="button" onClick={() => { setMenu(null); setWorkspaceMenu((value) => value === workspace.workspace_id ? null : workspace.workspace_id); }} aria-label={t(`Manage workspace ${workspace.workspace_name}`, `管理工作区 ${workspace.workspace_name}`)} title={t("Manage workspace", "管理工作区")}><MoreHorizontal size={14} /></button>
                )}
              </span>
              {workspaceMenu === workspace.workspace_id && (
                <div className="session-menu workspace-menu-popover" ref={menuRef}>
                  {canSelect && <button type="button" onClick={() => { setWorkspaceMenu(null); selectWorkspaceSessions(); }}><CheckSquare2 size={14} /> {t("Select sessions", "多选会话")}</button>}
                  {canClose && <button type="button" className="danger" onClick={() => { setWorkspaceMenu(null); void closeWorkspace(workspace.workspace_id, workspace.workspace_name, workspace.active); }}><X size={14} /> {t("Close workspace", "关闭工作区")}</button>}
                </div>
              )}
            </div>
            {workspaceExpanded && <div className="workspace-session-children">{sessions.map((session) => {
          const checked = selected.has(session.id);
          const running = runningSessions.has(`${workspace.workspace_id}:${session.id}`);
          return (
            <div className={`${session.active ? "session-row active" : "session-row"}${checked ? " selected" : ""}${running ? " running" : ""}`} key={session.id}>
              {selecting && workspace.active && (
                <button type="button" className="session-check" onClick={() => toggleSelected(session.id)} disabled={session.id === "default"} aria-label={t(`Select ${session.title}`, `选择 ${session.title}`)}>
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
                    if (session.id !== "default") toggleSelected(session.id);
                    return;
                  }
                  void openSession(workspace.workspace_id, session.id, workspace.active, session.active);
                }}>
                  <span><strong>{session.title}</strong><small>{new Date(session.updated_at).toLocaleString(locale)}</small></span>
                  {running && <ActiveAgentIndicator />}
                </button>
              )}
              {!selecting && workspace.active && renaming !== session.id && <button type="button" className="session-more" aria-label={t(`Manage ${session.title}`, `管理 ${session.title}`)} onClick={() => setMenu((value) => value === session.id ? null : session.id)}><MoreHorizontal size={15} /></button>}
              {!selecting && menu === session.id && (
                <div className="session-menu" ref={menuRef}>
                  <button type="button" onClick={() => startRename(session.id, session.title)}><Pencil size={14} /> {t("Rename", "重命名")}</button>
                  <button type="button" className="danger" disabled={session.id === "default"} onClick={() => { remove.mutate(session.id); setMenu(null); }}><Trash2 size={14} /> {t("Delete", "删除")}</button>
                </div>
              )}
            </div>
          );
        })}</div>}
          </div>;
        })}
      </div>
      {error && <p className="sidebar-error">{error.message}</p>}
      <div className="sidebar-footer" ref={appMenuRef}>
        <button
          type="button"
          className={`sidebar-settings-link${appMenuOpen || appMenuActive ? " active" : ""}`}
          onClick={() => setAppMenuOpen((value) => !value)}
          aria-expanded={appMenuOpen}
        >
          <Settings size={15} strokeWidth={1.8} /><span>{t("Application", "应用")}</span>
        </button>
        {appMenuOpen && (
          <div className="sidebar-app-popover">
            <button type="button" onClick={() => { setAppMenuOpen(false); setBrowserOpen(true); }}>
              <FolderOpen size={14} /><span>{t("Open server directory", "打开服务端目录")}</span>
            </button>
            <NavLink to="/settings" onClick={() => { setAppMenuOpen(false); onNavigate?.(); }} className={({ isActive }) => isActive ? "active" : ""}>
              <Settings size={14} /><span>{t("Settings", "配置")}</span>
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
    </div>
  );
}
