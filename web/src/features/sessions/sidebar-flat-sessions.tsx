import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { SessionRow } from "./session-row";
import { SessionRenameDialog } from "./session-rename-dialog";
import { SidebarSessionContextMenu } from "./sidebar-session-context-menu";
import { sessionActivityKey } from "./session-running-state";
import type { SidebarSessionRef } from "./sidebar-session-model";
import type { useSessionSelection } from "./use-session-selection";

type SelectionState = ReturnType<typeof useSessionSelection>;

type SidebarFlatSessionsProps = {
  items: SidebarSessionRef[];
  runningSessions: ReadonlySet<string>;
  now: number;
  archived?: boolean;
  empty: string;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  onRename: (id: string, title: string) => Promise<void>;
  onDelete: (id: string, title: string) => void;
  selection: SelectionState;
};

/**
 * 渲染不按项目分组的会话行，并共用右键菜单。
 *
 * @param props 会话、运行状态和操作回调
 * @returns 会话行列表
 */
export function SidebarFlatSessions({ items, runningSessions, now, archived = false, empty, onOpenSession, onRename, onDelete, selection }: SidebarFlatSessionsProps) {
  const index = useQuery({ queryKey: ["session-sidebar"], queryFn: () => api.sessionSidebar.read() });
  const [renaming, setRenaming] = useState<{ id: string; title: string } | null>(null);
  const [menu, setMenu] = useState<{ item: SidebarSessionRef; x: number; y: number } | null>(null);
  if (!items.length) return <p className="session-list-empty">{empty}</p>;
  return (
    <div className="sidebar-flat-sessions">
      {items.map((item) => (
        <SessionRow
          key={item.session.id}
          session={{ ...item.session, active: item.workspaceActive && item.session.active }}
          loaded={Boolean(item.session.loaded)}
          running={runningSessions.has(sessionActivityKey(item.workspaceId, item.session.id))}
          holder={item.session.holder}
          unread={Boolean(index.data?.unread[item.session.id])}
          now={now}
          selectable={selection.selecting && item.workspaceActive}
          checked={selection.selected.has(item.session.id)}
          canSelect={item.workspaceActive}
          canManage={true}
          onOpen={() => onOpenSession(item.workspaceId, item.session.id, item.workspaceActive, item.session.active)}
          onToggleChecked={() => selection.toggleSelected(item.session.id)}
          onStartRename={() => setRenaming({ id: item.session.id, title: item.session.title })}
          onEnterSelection={() => selection.enterSelection()}
          onDelete={() => onDelete(item.session.id, item.session.title)}
          onContextMenu={(event) => {
            event.preventDefault();
            setMenu({ item, x: event.clientX, y: event.clientY });
          }}
        />
      ))}
      {renaming && <SessionRenameDialog key={renaming.id} session={renaming} onRename={onRename} onClose={() => setRenaming(null)} />}
      {menu && (
        <SidebarSessionContextMenu
          sessionId={menu.item.session.id}
          title={menu.item.session.title}
          workspacePath={menu.item.workspacePath}
          x={menu.x}
          y={menu.y}
          archived={archived}
          onClose={() => setMenu(null)}
          onRename={() => setRenaming({ id: menu.item.session.id, title: menu.item.session.title })}
          onDelete={() => onDelete(menu.item.session.id, menu.item.session.title)}
          onEnterSelection={menu.item.workspaceActive ? () => {
            setMenu(null);
            selection.enterSelection();
          } : undefined}
        />
      )}
    </div>
  );
}
