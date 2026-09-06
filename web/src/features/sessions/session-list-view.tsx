import { X } from "lucide-react";
import { useState } from "react";
import type { RefObject } from "react";
import type { WorkspaceSessions } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { useI18n } from "../i18n/use-i18n";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { SessionWorkspaceIcon } from "./session-workspace-icon";
import { SessionRow } from "./session-row";
import { SessionSelectionBar } from "./session-selection-bar";
import { Button } from "../../shared/ui/button/button";
import type { useSessionSelection } from "./use-session-selection";
import { SessionRenameDialog } from "./session-rename-dialog";
import { sessionActivityKey } from "./session-running-state";

type SelectionState = ReturnType<typeof useSessionSelection>;

type SessionListViewProps = {
  showWorkspaceHeader?: boolean;
  workspace: WorkspaceSessions;
  runningSessions: ReadonlySet<string>;
  selection: SelectionState;
  /** 相对时间基准，由外层按分钟推进 */
  now: number;
  /** 菜单外点关闭用的容器引用 */
  menuRef: RefObject<HTMLDivElement | null>;
  /** 当前打开菜单的会话 ID */
  menu: string | null;
  onToggleMenu: (id: string | null) => void;
  onOpenSession: (sessionId: string, sessionActive: boolean) => void;
  onRename: (id: string, title: string) => Promise<void>;
  onDelete: (id: string, title: string) => void;
};

/**
 * 渲染活动工作区的会话列表视图。
 *
 * 顶部固定一行工作区标示：会话视图原先把工作区行整个藏掉，
 * 多工作区并行时看不出列表属于谁；标示行给出名称与会话数，
 * 不可点击，纯粹是上下文。
 *
 * @param props 工作区数据、多选状态与行操作回调
 * @returns 会话列表视图
 */
export function SessionListView({
  workspace,
  runningSessions,
  selection,
  now,
  onToggleMenu,
  onOpenSession,
  onRename,
  onDelete,
  showWorkspaceHeader = true
}: SessionListViewProps) {
  const { locale, t } = useI18n();
  const [renaming, setRenaming] = useState<{ id: string; title: string } | null>(null);
  const [showAll, setShowAll] = useState(false);

  const workspaceName = localizeApiMessage(workspace.workspace_name, locale);
  const sessions = workspace.sessions;
  const selecting = selection.selecting && workspace.active;
  const visibleSessions = showAll || selecting || showWorkspaceHeader ? sessions : sessions.filter((session, index) => index < 8 || (workspace.active && session.active));
  const workspaceRunning = sessions.some((session) => runningSessions.has(sessionActivityKey(workspace.workspace_id, session.id)));

  /**
   * 进入指定会话的重命名编辑态。
   *
   * @param id 会话 ID
   * @param title 当前标题
   */
  const startRename = (id: string, title: string) => {
    setRenaming({ id, title });
    onToggleMenu(null);
  };

  return (
    <div className="session-list sidebar-sessions-view">
      {(showWorkspaceHeader || selecting) && <div className="workspace-context-row">
        <SessionWorkspaceIcon isGitRepository={workspace.is_git_repository} size={13} />
        <strong title={workspace.workspace_path}>{workspaceName}</strong>
        {workspaceRunning && <ActiveAgentIndicator />}
        <small>{t(`${sessions.length} sessions`, `${sessions.length} 个会话`)}</small>
        {selecting && (
          <button
            type="button"
            className="workspace-context-exit"
            onClick={selection.exitSelection}
            aria-label={t("Exit selection", "退出选择")}
            title={t("Exit selection", "退出选择")}
          >
            <X size={13} />
          </button>
        )}
      </div>}
      {selecting && (
        <SessionSelectionBar
          sessionIds={sessions.map((session) => session.id)}
          selectedCount={selection.selected.size}
          confirming={selection.confirming}
          busy={selection.busy}
          onToggleAll={selection.toggleAll}
          onDelete={() => void selection.requestBulkDelete()}
        />
      )}
      <div className="workspace-session-children">
        {sessions.length === 0 && (
          <p className="session-list-empty">{t("No sessions yet. Create a task to start.", "还没有会话。新建任务开始对话。")}</p>
        )}
        {visibleSessions.map((session) => (
          <SessionRow
            key={session.id}
            session={{ ...session, active: workspace.active && session.active }}
            loaded={Boolean(session.loaded)}
            running={runningSessions.has(sessionActivityKey(workspace.workspace_id, session.id))}
            holder={session.holder}
            now={now}
            selectable={selecting}
            checked={selection.selected.has(session.id)}
            canSelect={sessions.length > 0}
            canManage={workspace.active}
            onOpen={() => onOpenSession(session.id, session.active)}
            onToggleChecked={() => selection.toggleSelected(session.id)}
            onStartRename={() => startRename(session.id, session.title)}
            onEnterSelection={() => {
              onToggleMenu(null);
              selection.enterSelection();
            }}
            onDelete={() => {
              onToggleMenu(null);
              onDelete(session.id, session.title);
            }}
          />
        ))}
        {sessions.length > 8 && !selecting && !showWorkspaceHeader && <Button variant="ghost" className="sidebar-show-more" onClick={() => setShowAll((value) => !value)}>{showAll ? t("Show less", "收起") : t(`Show all ${sessions.length} tasks`, `显示全部 ${sessions.length} 个任务`)}</Button>}
      </div>
      {renaming && <SessionRenameDialog key={renaming.id} session={renaming} onRename={onRename} onClose={() => setRenaming(null)} />}
    </div>
  );
}
