import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Hash, Plus } from "lucide-react";
import { api, type SessionSidebarGroup } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { TextInput } from "../../shared/ui/form/text-input";
import { useI18n } from "../i18n/use-i18n";
import { SidebarFlatSessions } from "./sidebar-flat-sessions";
import type { SidebarSessionRef } from "./sidebar-session-model";
import type { useSessionSelection } from "./use-session-selection";

type SelectionState = ReturnType<typeof useSessionSelection>;

const GROUP_COLORS = ["signal", "warning", "danger", "ink"];

type SidebarGroupedSectionProps = {
  items: SidebarSessionRef[];
  runningSessions: ReadonlySet<string>;
  now: number;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  onRename: (id: string, title: string) => Promise<void>;
  onDelete: (id: string, title: string) => void;
  selection: SelectionState;
};

/**
 * 按用户分组渲染会话。未进入任何分组的会话留在「未分组」。
 *
 * @param props 可分组的会话
 * @returns 分组列表
 */
export function SidebarGroupedSection({ items, runningSessions, now, onOpenSession, onRename, onDelete, selection }: SidebarGroupedSectionProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const index = useQuery({ queryKey: ["session-sidebar"], queryFn: () => api.sessionSidebar.read() });
  const [draft, setDraft] = useState("");
  const [creating, setCreating] = useState(false);
  const groups = index.data?.groups ?? [];
  const groupedIds = new Set(groups.flatMap((group) => group.session_ids));
  const ungrouped = items.filter((item) => !groupedIds.has(item.session.id));

  /**
   * 写回分组列表。
   *
   * @param next 新分组
   */
  const saveGroups = (next: SessionSidebarGroup[]) => {
    void api.sessionSidebar.update({ groups: next }).then(() => queryClient.invalidateQueries({ queryKey: ["session-sidebar"] }));
  };

  /**
   * 用输入框中的名称新建分组。
   */
  const createGroup = () => {
    const name = draft.trim();
    if (!name) return;
    saveGroups([...groups, {
      id: `group_${Date.now()}`,
      name,
      color: GROUP_COLORS[groups.length % GROUP_COLORS.length],
      session_ids: []
    }]);
    setDraft("");
    setCreating(false);
  };

  return (
    <div className="sidebar-groups">
      <div className="sidebar-groups-create">
        {creating ? (
          <TextInput
            autoFocus
            value={draft}
            placeholder={t("Group name", "分组名称")}
            aria-label={t("Group name", "分组名称")}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") createGroup();
              if (event.key === "Escape") setCreating(false);
            }}
          />
        ) : (
          <Button variant="ghost" onClick={() => setCreating(true)}><Plus size={14} />{t("New group", "新建分组")}</Button>
        )}
      </div>
      {groups.map((group) => {
        const members = group.session_ids.flatMap((id) => items.filter((item) => item.session.id === id));
        return (
          <section key={group.id} className={`sidebar-group color-${group.color}`}>
            <h3><Hash size={12} /><span>{group.name}</span><small>{members.length}</small></h3>
            <SidebarFlatSessions
              items={members}
              runningSessions={runningSessions}
              now={now}
              empty={t("Drop a task here from its menu", "从任务菜单把会话移到这里")}
              onOpenSession={onOpenSession}
              onRename={onRename}
              onDelete={onDelete}
              selection={selection}
            />
          </section>
        );
      })}
      <section className="sidebar-group">
        <h3>{t("Ungrouped", "未分组")}</h3>
        <SidebarFlatSessions items={ungrouped} runningSessions={runningSessions} now={now} empty={t("No tasks yet", "还没有任务")} onOpenSession={onOpenSession} onRename={onRename} onDelete={onDelete} selection={selection} />
      </section>
    </div>
  );
}
