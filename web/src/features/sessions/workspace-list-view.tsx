import { MoreHorizontal, Plus, X } from "lucide-react";
import type { RefObject } from "react";
import type { WorkspaceSessions } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { useI18n } from "../i18n/use-i18n";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { SessionWorkspaceIcon } from "./session-workspace-icon";

type WorkspaceListViewProps = {
  workspaces: WorkspaceSessions[];
  runningSessions: Set<string>;
  /** 菜单外点关闭用的容器引用 */
  menuRef: RefObject<HTMLDivElement | null>;
  /** 当前打开菜单的工作区 ID */
  menu: string | null;
  onToggleMenu: (id: string | null) => void;
  onOpenWorkspace: (workspaceId: string, active: boolean) => void;
  onCreateSession: (workspaceId: string, active: boolean) => void;
  createPending: boolean;
  onCloseWorkspace: (workspaceId: string, name: string, active: boolean) => void;
};

/**
 * 渲染工作区视图：全部工作区的切换列表。
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
  onOpenWorkspace,
  onCreateSession,
  createPending,
  onCloseWorkspace
}: WorkspaceListViewProps) {
  const { locale, t } = useI18n();
  const canClose = workspaces.length > 1;
  return (
    <div className="session-list sidebar-workspaces-view">
      {workspaces.map((workspace) => {
        const name = localizeApiMessage(workspace.workspace_name, locale);
        const running = workspace.sessions.some((session) =>
          runningSessions.has(`${workspace.workspace_id}:${session.id}`)
        );
        return (
          <div className="session-workspace" key={workspace.workspace_id}>
            <div className={workspace.active ? "workspace-tree-row active" : "workspace-tree-row"}>
              <button
                type="button"
                className="workspace-tree-main"
                onClick={() => onOpenWorkspace(workspace.workspace_id, workspace.active)}
                title={workspace.workspace_path}
              >
                <SessionWorkspaceIcon isGitRepository={workspace.is_git_repository} size={14} />
                <span className="workspace-summary">
                  <strong>{name}</strong>
                  {running && <ActiveAgentIndicator />}
                  <small>{t(`${workspace.sessions.length} sessions`, `${workspace.sessions.length} 个会话`)}</small>
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
                {canClose && (
                  <button
                    type="button"
                    onClick={() => onToggleMenu(menu === workspace.workspace_id ? null : workspace.workspace_id)}
                    aria-label={t(`Manage workspace ${name}`, `管理工作区 ${name}`)}
                    title={t("Manage workspace", "管理工作区")}
                  >
                    <MoreHorizontal size={14} />
                  </button>
                )}
              </span>
              {menu === workspace.workspace_id && (
                <div className="session-menu workspace-menu-popover" ref={menuRef}>
                  <button
                    type="button"
                    className="danger"
                    onClick={() => {
                      onToggleMenu(null);
                      onCloseWorkspace(workspace.workspace_id, name, workspace.active);
                    }}
                  >
                    <X size={14} /> {t("Close workspace", "关闭工作区")}
                  </button>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
