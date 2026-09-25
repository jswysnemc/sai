import type { MouseEvent } from "react";
import type { WorkspaceSessions } from "../../api/contracts";
import { formatRelativeTime } from "../../shared/format-relative-time";
import { useI18n } from "../i18n/use-i18n";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { sessionActivityKey } from "./session-running-state";
import { sidebarSessionKey } from "./sidebar-session-key";

type WorkspaceSessionListProps = {
  workspace: WorkspaceSessions;
  runningSessions: ReadonlySet<string>;
  now: number;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  onContextMenu?: (sessionId: string, title: string, event: MouseEvent) => void;
};

/**
 * 渲染工作区展开后的会话。
 *
 * 点击一条会话会激活它；当前会话用活动态标出。
 *
 * @param props 工作区、运行状态和打开回调
 * @returns 会话列表
 */
export function WorkspaceSessionList({ workspace, runningSessions, now, onOpenSession, onContextMenu }: WorkspaceSessionListProps) {
  const { locale, t } = useI18n();
  const sessions = [...workspace.sessions].sort((left, right) => right.updated_at.localeCompare(left.updated_at));
  if (sessions.length === 0) {
    return <p className="workspace-session-empty">{t("No sessions yet", "还没有会话")}</p>;
  }
  return (
    <div className="workspace-session-list">
      {sessions.map((session) => {
        const active = workspace.active && session.active;
        const running = runningSessions.has(sessionActivityKey(workspace.workspace_id, session.id));
        return (
          <button
            key={sidebarSessionKey(workspace.workspace_id, session.id)}
            type="button"
            className={active ? "workspace-session-item active" : "workspace-session-item"}
            aria-current={active ? "page" : undefined}
            onClick={() => onOpenSession(workspace.workspace_id, session.id, workspace.active, session.active)}
            onContextMenu={(event) => onContextMenu?.(session.id, session.title, event)}
          >
            <span>{session.title}</span>
            {running && <ActiveAgentIndicator holder={session.holder} />}
            <small className="workspace-session-time" title={new Date(session.updated_at).toLocaleString(locale)}>{formatRelativeTime(session.updated_at, locale, now)}</small>
          </button>
        );
      })}
    </div>
  );
}
