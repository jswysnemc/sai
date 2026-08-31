import { X } from "lucide-react";
import { useRef, useState } from "react";
import type { RefObject } from "react";
import type { WorkspaceSessions } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { useI18n } from "../i18n/use-i18n";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { SessionWorkspaceIcon } from "./session-workspace-icon";
import { SessionRow } from "./session-row";
import { SessionSelectionBar } from "./session-selection-bar";
import type { useSessionSelection } from "./use-session-selection";

type SelectionState = ReturnType<typeof useSessionSelection>;

type SessionListViewProps = {
  workspace: WorkspaceSessions;
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
  selection,
  now,
  menuRef,
  menu,
  onToggleMenu,
  onOpenSession,
  onRename,
  onDelete
}: SessionListViewProps) {
  const { locale, t } = useI18n();
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [renamePending, setRenamePending] = useState(false);
  // 提交后异步返回前若用户已开始编辑别的行，不能把新的编辑态清掉
  const renameTarget = useRef<string | null>(null);

  const workspaceName = localizeApiMessage(workspace.workspace_name, locale);
  const sessions = workspace.sessions;
  const workspaceLoaded = sessions.some((session) => session.loaded);

  /**
   * 进入指定会话的重命名编辑态。
   *
   * @param id 会话 ID
   * @param title 当前标题
   */
  const startRename = (id: string, title: string) => {
    setRenaming(id);
    setRenameDraft(title);
    onToggleMenu(null);
  };

  /** 提交重命名，标题为空或未变化时直接退出编辑态。 */
  const submitRename = async () => {
    if (!renaming) return;
    const title = renameDraft.trim();
    const current = sessions.find((session) => session.id === renaming);
    if (!title || title === current?.title) {
      setRenaming(null);
      return;
    }
    renameTarget.current = renaming;
    setRenamePending(true);
    try {
      await onRename(renaming, title);
    } finally {
      setRenamePending(false);
      setRenaming((value) => (value === renameTarget.current ? null : value));
    }
  };

  return (
    <div className="session-list sidebar-sessions-view">
      <div className="workspace-context-row">
        <SessionWorkspaceIcon isGitRepository={workspace.is_git_repository} size={13} />
        <strong title={workspace.workspace_path}>{workspaceName}</strong>
        {workspaceLoaded && <ActiveAgentIndicator />}
        <small>{t(`${sessions.length} sessions`, `${sessions.length} 个会话`)}</small>
        {selection.selecting && (
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
      </div>
      {selection.selecting && (
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
        {sessions.map((session) => (
          <SessionRow
            key={session.id}
            session={session}
            loaded={Boolean(session.loaded)}
            holder={session.holder}
            now={now}
            selectable={selection.selecting}
            checked={selection.selected.has(session.id)}
            renaming={renaming === session.id}
            renameDraft={renameDraft}
            renamePending={renamePending}
            menuOpen={menu === session.id}
            menuRef={menuRef}
            canSelect={sessions.length > 0}
            onOpen={() => onOpenSession(session.id, session.active)}
            onToggleChecked={() => selection.toggleSelected(session.id)}
            onToggleMenu={() => onToggleMenu(menu === session.id ? null : session.id)}
            onStartRename={() => startRename(session.id, session.title)}
            onRenameDraft={setRenameDraft}
            onRenameSubmit={() => void submitRename()}
            onRenameCancel={() => setRenaming(null)}
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
      </div>
    </div>
  );
}
