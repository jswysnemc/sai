import { ChevronRight, Plus } from "lucide-react";
import { useEffect, useState, type ReactNode, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { WorkspaceSessions } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { SkeletonList } from "../../shared/ui/skeleton/skeleton";
import { useI18n } from "../i18n/use-i18n";
import { SidebarPinnedSection } from "./sidebar-pinned-section";
import { SessionSelectionBar } from "./session-selection-bar";
import { flattenSessions, sortSessions } from "./sidebar-session-model";
import { countSessionIds, matchesStoredSessionId, sidebarSessionKey } from "./sidebar-session-key";
import { readExpandedWorkspaces, toggleWorkspaceExpanded, withActiveWorkspaceExpanded, writeExpandedWorkspaces } from "./workspace-expansion-state";
import { SidebarTaskToolbar } from "./sidebar-task-toolbar";
import { captureRecentBaseline, rememberRecent, readRecents, splitByRecent, type SidebarBrowseMode, type SidebarRecents } from "./sidebar-browse";
import { SidebarFlatSessions } from "./sidebar-flat-sessions";
import { WorkspaceListView } from "./workspace-list-view";
import type { useSessionSelection } from "./use-session-selection";
import type { SessionScope } from "./session-scope-control";
import "./sidebar-task-board.css";

type SelectionState = ReturnType<typeof useSessionSelection>;

type SidebarTaskBoardProps = {
  workspaces: WorkspaceSessions[];
  loading: boolean;
  runningSessions: ReadonlySet<string>;
  now: number;
  sessionScope: SessionScope;
  selection: SelectionState;
  menuRef: RefObject<HTMLDivElement | null>;
  menu: string | null;
  workspaceMenu: string | null;
  createPending: boolean;
  browse: SidebarBrowseMode;
  onBrowse: (mode: SidebarBrowseMode) => void;
  selectedSessionId?: string;
  files?: ReactNode;
  onToggleMenu: (id: string | null) => void;
  onToggleWorkspaceMenu: (id: string | null) => void;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  onRename: (id: string, title: string) => Promise<void>;
  onDelete: (id: string, title: string) => void;
  onCreateSession: (workspaceId: string, active: boolean) => void;
  onCloseWorkspace: (workspaceId: string, name: string, active: boolean) => void;
  onAddWorkspace: () => void;
};

/**
 * 左栏浏览：最近三条会话或工作区，其余折进历史，文件树单独一页。
 *
 * @param props 工作区数据、浏览模式和会话操作
 * @returns 左栏任务区
 */
export function SidebarTaskBoard(props: SidebarTaskBoardProps) {
  const { t } = useI18n();
  const [recents, setRecents] = useState<SidebarRecents>(readRecents);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [expandedWorkspaces, setExpandedWorkspaces] = useState<ReadonlySet<string>>(readExpandedWorkspaces);
  const index = useQuery({ queryKey: ["session-sidebar"], queryFn: () => api.sessionSidebar.read() });
  const activeWorkspaceId = props.workspaces.find((workspace) => workspace.active)?.workspace_id;

  useEffect(() => {
    if (!props.selectedSessionId || !activeWorkspaceId) return;
    setRecents(rememberRecent("sessions", sidebarSessionKey(activeWorkspaceId, props.selectedSessionId)));
  }, [props.selectedSessionId, activeWorkspaceId]);

  useEffect(() => {
    if (!activeWorkspaceId) return;
    setRecents(rememberRecent("workspaces", activeWorkspaceId));
    setExpandedWorkspaces((current) => {
      const next = withActiveWorkspaceExpanded(current, activeWorkspaceId);
      if (next.size === current.size && [...next].every((id) => current.has(id))) return current;
      writeExpandedWorkspaces(next);
      return next;
    });
  }, [activeWorkspaceId]);

  useEffect(() => {
    setHistoryOpen(false);
  }, [props.browse]);

  const sessions = sortSessions(flattenSessions(props.workspaces), "updated");
  const sessionIdCounts = countSessionIds(sessions.map((item) => item.session.id));
  const pinnedIds = index.data?.pinned ?? [];
  const isPinned = (item: (typeof sessions)[number]) => pinnedIds.some((id) =>
    matchesStoredSessionId(id, item.workspaceId, item.session.id, sessionIdCounts.get(item.session.id) ?? 1)
  );
  const pinned = sessions.filter(isPinned);
  const unpinnedSessions = sessions.filter((item) => !isPinned(item));
  const sessionSplit = splitByRecent(
    unpinnedSessions,
    (item) => sidebarSessionKey(item.workspaceId, item.session.id),
    recents.sessions,
    recents.sessionBaseline
  );
  const workspacesByOpen = [...props.workspaces].sort((left, right) => right.last_opened_at.localeCompare(left.last_opened_at));
  const workspaceSplit = splitByRecent(workspacesByOpen, (workspace) => workspace.workspace_id, recents.workspaces, recents.workspaceBaseline);
  const sessionOrder = unpinnedSessions.map((item) => sidebarSessionKey(item.workspaceId, item.session.id)).join("|");
  const workspaceOrder = workspacesByOpen.map((workspace) => workspace.workspace_id).join("|");

  useEffect(() => {
    const next = captureRecentBaseline("sessionBaseline", sessionOrder.split("|").filter(Boolean));
    if (next) setRecents(next);
  }, [sessionOrder]);

  useEffect(() => {
    const next = captureRecentBaseline("workspaceBaseline", workspaceOrder.split("|").filter(Boolean));
    if (next) setRecents(next);
  }, [workspaceOrder]);

  const selectableIds = props.workspaces.find((workspace) => workspace.active)?.sessions.map((session) => session.id) ?? [];
  const rowActions = {
    runningSessions: props.runningSessions,
    now: props.now,
    onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => {
      setRecents(rememberRecent("sessions", sidebarSessionKey(workspaceId, sessionId)));
      props.onOpenSession(workspaceId, sessionId, workspaceActive, sessionActive);
    },
    onRename: props.onRename,
    onDelete: props.onDelete,
    selection: props.selection,
    sessionIdCounts
  };

  /**
   * 展开或收起一个工作区，并记住这个选择。
   *
   * @param workspaceId 工作区 ID
   */
  const toggleExpanded = (workspaceId: string) => {
    setExpandedWorkspaces((current) => {
      const next = toggleWorkspaceExpanded(current, workspaceId);
      writeExpandedWorkspaces(next);
      return next;
    });
  };

  return (
    <div className={`sidebar-purpose-scroll${props.browse === "files" ? " is-files" : ""}`}>
      <div className="sidebar-browse-bar">
        <SidebarTaskToolbar mode={props.browse} onChange={props.onBrowse} />
        {props.browse === "workspaces" && (
          <Button variant="ghost" size="icon" aria-label={t("Add workspace", "加入工作区")} title={t("Add workspace", "加入工作区")} onClick={props.onAddWorkspace}>
            <Plus size={14} />
          </Button>
        )}
      </div>
      {props.selection.selecting && props.browse === "sessions" && (
        <div className="sidebar-selection-host">
          <SessionSelectionBar
            sessionIds={selectableIds}
            selectedCount={props.selection.selected.size}
            confirming={props.selection.confirming}
            busy={props.selection.busy}
            onToggleAll={props.selection.toggleAll}
            onDelete={() => void props.selection.requestBulkDelete()}
          />
          <button type="button" className="workspace-context-exit" onClick={props.selection.exitSelection} aria-label={t("Exit selection", "退出选择")} title={t("Exit selection", "退出选择")}>
            ×
          </button>
        </div>
      )}
      {props.browse === "files" && <div className="sidebar-file-tree-view">{props.files}</div>}
      {props.browse === "sessions" && (props.loading ? <SkeletonList items={4} label={t("Loading sessions", "读取会话")} /> : (
        <>
          <SidebarPinnedSection items={pinned} {...rowActions} />
          {sessionSplit.recent.length > 0 ? (
            <div className="sidebar-recent-list">
              <SidebarFlatSessions items={sessionSplit.recent} empty="" {...rowActions} />
            </div>
          ) : pinned.length === 0 ? <p className="session-list-empty">{t("No sessions yet", "还没有会话")}</p> : null}
          <HistoryFold
            title={t("Session history", "历史会话")}
            count={sessionSplit.history.length}
            open={historyOpen}
            onOpenChange={setHistoryOpen}
          >
            <SidebarFlatSessions items={sessionSplit.history} empty="" {...rowActions} />
          </HistoryFold>
        </>
      ))}
      {props.browse === "workspaces" && (props.loading ? <SkeletonList items={3} label={t("Loading sessions", "读取会话")} /> : props.workspaces.length > 0 ? (
        <>
          <WorkspaceListView
            workspaces={workspaceSplit.recent}
            preserveOrder
            allowClose={props.workspaces.length > 1}
            runningSessions={props.runningSessions}
            menuRef={props.menuRef}
            menu={props.workspaceMenu}
            onToggleMenu={props.onToggleWorkspaceMenu}
            onOpenSession={rowActions.onOpenSession}
            expandedWorkspaceIds={expandedWorkspaces}
            onToggleExpanded={toggleExpanded}
            onCreateSession={props.onCreateSession}
            createPending={props.createPending}
            now={props.now}
            onCloseWorkspace={props.onCloseWorkspace}
            onRenameSession={props.onRename}
            onDeleteSession={props.onDelete}
          />
          <HistoryFold
            title={t("Workspace history", "历史工作区")}
            count={workspaceSplit.history.length}
            open={historyOpen}
            onOpenChange={setHistoryOpen}
          >
            <WorkspaceListView
              workspaces={workspaceSplit.history}
              preserveOrder
              allowClose={props.workspaces.length > 1}
              runningSessions={props.runningSessions}
              menuRef={props.menuRef}
              menu={props.workspaceMenu}
              onToggleMenu={props.onToggleWorkspaceMenu}
              onOpenSession={rowActions.onOpenSession}
              expandedWorkspaceIds={expandedWorkspaces}
              onToggleExpanded={toggleExpanded}
              onCreateSession={props.onCreateSession}
              createPending={props.createPending}
              now={props.now}
              onCloseWorkspace={props.onCloseWorkspace}
              onRenameSession={props.onRename}
              onDeleteSession={props.onDelete}
            />
          </HistoryFold>
        </>
      ) : <div className="sidebar-state">{t("No projects yet", "尚未打开项目")}</div>)}
    </div>
  );
}

type HistoryFoldProps = {
  title: string;
  count: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
};

/**
 * 把超出最近三条的条目收进可展开的历史区。
 *
 * @param props 标题、数量和展开状态
 * @returns 历史折叠，没有条目时不渲染
 */
function HistoryFold({ title, count, open, onOpenChange, children }: HistoryFoldProps) {
  if (count === 0) return null;
  return (
    <section className="sidebar-history">
      <button type="button" aria-expanded={open} onClick={() => onOpenChange(!open)}>
        <ChevronRight size={14} data-open={open} />
        <span>{title}</span>
        <small>{count}</small>
      </button>
      {open && children}
    </section>
  );
}
