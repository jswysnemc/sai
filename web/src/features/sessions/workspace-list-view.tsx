import { ChevronRight, MoreHorizontal, Plus } from "lucide-react";
import { useState, type RefObject } from "react";
import type { WorkspaceSessions } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { formatRelativeTime } from "../../shared/format-relative-time";
import { useI18n } from "../i18n/use-i18n";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { SessionWorkspaceHovercard } from "./session-workspace-hovercard";
import { SessionWorkspaceIcon } from "./session-workspace-icon";
import { sessionActivityKey } from "./session-running-state";
import { SessionRenameDialog } from "./session-rename-dialog";
import { SidebarSessionContextMenu } from "./sidebar-session-context-menu";
import { WorkspaceContextMenu } from "./workspace-context-menu";
import { WorkspaceSessionList } from "./workspace-session-list";

type WorkspaceListViewProps = {
  workspaces: WorkspaceSessions[];
  runningSessions: ReadonlySet<string>;
  /** 菜单外点关闭用的容器引用 */
  menuRef: RefObject<HTMLDivElement | null>;
  /** 当前打开菜单的工作区 ID */
  menu: string | null;
  onToggleMenu: (id: string | null) => void;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  expandedWorkspaceIds: ReadonlySet<string>;
  onToggleExpanded: (workspaceId: string) => void;
  onCreateSession: (workspaceId: string, active: boolean) => void;
  createPending: boolean;
  now: number;
  onCloseWorkspace: (workspaceId: string, name: string, active: boolean) => void;
  onRenameSession?: (id: string, title: string) => Promise<void>;
  onDeleteSession?: (id: string, title: string) => void;
  /** 为真时保持传入顺序，不再按最后打开时间重排。 */
  preserveOrder?: boolean;
  /** 覆盖“列表多于一个才可关闭”。最近三条和历史要按全部工作区判断。 */
  allowClose?: boolean;
};

/**
 * 渲染工作区视图：点击工作区只展开或收起会话，点击会话才激活工作区。
 *
 * 每行给出名称、运行状态与会话数；行内操作只保留「在此新建会话」
 * 与管理菜单。多选会话属于会话列表的操作，不再出现在这里——
 * 原实现从这里也能进入多选，但工作区视图不渲染会话行，
 * 进入后只有一条悬空的工具条。
 *
 * @param props 工作区数据与行操作回调
 * @returns 工作区列表视图
 */
export function WorkspaceListView({
  workspaces,
  runningSessions,
  menuRef,
  menu,
  onToggleMenu,
  onOpenSession,
  expandedWorkspaceIds,
  onToggleExpanded,
  onCreateSession,
  createPending,
  now,
  onCloseWorkspace,
  onRenameSession,
  onDeleteSession,
  preserveOrder = false,
  allowClose
}: WorkspaceListViewProps) {
  const { locale, t } = useI18n();
  const canClose = allowClose ?? workspaces.length > 1;
  const [workspaceMenuPoint, setWorkspaceMenuPoint] = useState<{
    id: string;
    name: string;
    path: string;
    x: number;
    y: number;
  } | null>(null);
  const [sessionMenuPoint, setSessionMenuPoint] = useState<{
    id: string;
    workspaceId: string;
    title: string;
    path: string;
    x: number;
    y: number;
  } | null>(null);
  const [renaming, setRenaming] = useState<{ id: string; title: string } | null>(null);
  const ordered = preserveOrder ? workspaces : [...workspaces].sort((left, right) => right.last_opened_at.localeCompare(left.last_opened_at));

  /**
   * 打开工作区菜单，并关掉仍挂在父级的旧弹出层。
   *
   * @param id 工作区 ID
   * @param name 工作区名称
   * @param path 工作区路径
   * @param x 视口横坐标
   * @param y 视口纵坐标
   */
  const openWorkspaceMenu = (id: string, name: string, path: string, x: number, y: number) => {
    onToggleMenu(null);
    setSessionMenuPoint(null);
    setWorkspaceMenuPoint({ id, name, path, x, y });
  };

  return (
    <div className="session-list sidebar-workspaces-view" ref={menu ? menuRef : undefined}>
      {ordered.map((workspace) => {
        const name = localizeApiMessage(workspace.workspace_name, locale);
        const running = workspace.sessions.some((session) => runningSessions.has(sessionActivityKey(workspace.workspace_id, session.id)));
        const expanded = expandedWorkspaceIds.has(workspace.workspace_id);
        return (
          <div className="session-workspace" key={workspace.workspace_id}>
            <SessionWorkspaceHovercard name={name} path={workspace.workspace_path}>
              <div
                className={workspace.active ? "workspace-tree-row active" : "workspace-tree-row"}
                onContextMenu={(event) => {
                  event.preventDefault();
                  openWorkspaceMenu(workspace.workspace_id, name, workspace.workspace_path, event.clientX, event.clientY);
                }}
              >
                <button
                  type="button"
                  className="workspace-expand"
                  aria-expanded={expanded}
                  aria-label={expanded ? t(`Collapse ${name}`, `收起 ${name}`) : t(`Expand ${name}`, `展开 ${name}`)}
                  onClick={() => onToggleExpanded(workspace.workspace_id)}
                >
                  <ChevronRight size={14} data-open={expanded} />
                </button>
                <button
                  type="button"
                  className="workspace-tree-main"
                  aria-expanded={expanded}
                  onClick={() => {
                    // #region agent log
                    fetch('http://127.0.0.1:7368/ingest/77461c80-9be3-44e4-ac14-3725f6920049',{method:'POST',headers:{'Content-Type':'application/json','X-Debug-Session-Id':'ff618c'},body:JSON.stringify({sessionId:'ff618c',hypothesisId:'W',location:'workspace-list-view.tsx:workspace-row',message:'workspace row toggles sessions without activating',data:{workspaceId:workspace.workspace_id,expanded,active:workspace.active,activate:false},timestamp:Date.now()})}).catch(()=>{});
                    // #endregion
                    onToggleExpanded(workspace.workspace_id);
                  }}
                >
                  <SessionWorkspaceIcon isGitRepository={workspace.is_git_repository} size={14} />
                  <span className="workspace-summary">
                    <strong>{name}</strong>
                    {running && <ActiveAgentIndicator />}
                    <span className="workspace-meta">
                      {!expanded && (
                        <small className="workspace-time" title={new Date(workspace.last_opened_at).toLocaleString(locale)}>{formatRelativeTime(workspace.last_opened_at, locale, now)}</small>
                      )}
                      <small className="workspace-count">{t(`${workspace.sessions.length} sessions`, `${workspace.sessions.length} 个会话`)}</small>
                    </span>
                  </span>
                </button>
                <span className="workspace-tree-actions">
                  <button
                    type="button"
                    className="workspace-create-session"
                    onClick={() => onCreateSession(workspace.workspace_id, workspace.active)}
                    disabled={createPending}
                    aria-label={t(`Create a session in ${name}`, `在 ${name} 新建会话`)}
                    title={t("New session", "新建会话")}
                  >
                    <Plus size={14} />
                  </button>
                  <button
                    type="button"
                    onClick={(event) => {
                      const rect = event.currentTarget.getBoundingClientRect();
                      openWorkspaceMenu(workspace.workspace_id, name, workspace.workspace_path, rect.left, rect.bottom);
                    }}
                    aria-label={t(`Manage workspace ${name}`, `管理工作区 ${name}`)}
                    title={t("Manage workspace", "管理工作区")}
                  >
                    <MoreHorizontal size={14} />
                  </button>
                </span>
              </div>
            </SessionWorkspaceHovercard>
            {expanded && (
              <WorkspaceSessionList
                workspace={workspace}
                runningSessions={runningSessions}
                now={now}
                onOpenSession={onOpenSession}
                onContextMenu={(sessionId, title, event) => {
                  event.preventDefault();
                  setWorkspaceMenuPoint(null);
                  setSessionMenuPoint({
                    id: sessionId,
                    workspaceId: workspace.workspace_id,
                    title,
                    path: workspace.workspace_path,
                    x: event.clientX,
                    y: event.clientY
                  });
                }}
              />
            )}
          </div>
        );
      })}
      {workspaceMenuPoint && (
        <WorkspaceContextMenu
          name={workspaceMenuPoint.name}
          path={workspaceMenuPoint.path}
          x={workspaceMenuPoint.x}
          y={workspaceMenuPoint.y}
          expanded={expandedWorkspaceIds.has(workspaceMenuPoint.id)}
          canClose={canClose}
          createPending={createPending}
          onClose={() => setWorkspaceMenuPoint(null)}
          onCreateSession={() => onCreateSession(workspaceMenuPoint.id, ordered.some((item) => item.workspace_id === workspaceMenuPoint.id && item.active))}
          onToggleExpanded={() => onToggleExpanded(workspaceMenuPoint.id)}
          onCloseWorkspace={() => {
            const current = ordered.find((item) => item.workspace_id === workspaceMenuPoint.id);
            onCloseWorkspace(workspaceMenuPoint.id, workspaceMenuPoint.name, current?.active ?? false);
          }}
        />
      )}
      {sessionMenuPoint && onRenameSession && onDeleteSession && (
        <SidebarSessionContextMenu
          sessionId={sessionMenuPoint.id}
          workspaceId={sessionMenuPoint.workspaceId}
          title={sessionMenuPoint.title}
          workspacePath={sessionMenuPoint.path}
          x={sessionMenuPoint.x}
          y={sessionMenuPoint.y}
          archived={false}
          onClose={() => setSessionMenuPoint(null)}
          onRename={() => setRenaming({ id: sessionMenuPoint.id, title: sessionMenuPoint.title })}
          onDelete={() => onDeleteSession(sessionMenuPoint.id, sessionMenuPoint.title)}
        />
      )}
      {renaming && onRenameSession && (
        <SessionRenameDialog
          session={renaming}
          onRename={onRenameSession}
          onClose={() => setRenaming(null)}
        />
      )}
    </div>
  );
}
