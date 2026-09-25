import { useState } from "react";
import { useI18n } from "../i18n/use-i18n";
import { SidebarFlatSessions } from "./sidebar-flat-sessions";
import type { SidebarSessionRef } from "./sidebar-session-model";
import type { useSessionSelection } from "./use-session-selection";

type SelectionState = ReturnType<typeof useSessionSelection>;

const PAGE_SIZE = 20;

type SidebarPinnedSectionProps = {
  items: SidebarSessionRef[];
  runningSessions: ReadonlySet<string>;
  now: number;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  onRename: (id: string, title: string) => Promise<void>;
  onDelete: (id: string, title: string) => void;
  selection: SelectionState;
  sessionIdCounts?: ReadonlyMap<string, number>;
};

/**
 * 渲染跨项目的置顶会话。没有置顶时不占高度。
 *
 * @param props 置顶会话和行操作
 * @returns 置顶分区
 */
export function SidebarPinnedSection({ items, runningSessions, now, onOpenSession, onRename, onDelete, selection, sessionIdCounts }: SidebarPinnedSectionProps) {
  const { t } = useI18n();
  const [limit, setLimit] = useState(PAGE_SIZE);
  if (!items.length) return null;
  const visible = items.slice(0, limit);
  return (
    <section className="sidebar-pinned" aria-label={t("Pinned", "已置顶")}>
      <h3>{t("Pinned", "已置顶")}</h3>
      <SidebarFlatSessions
        items={visible}
        runningSessions={runningSessions}
        now={now}
        empty=""
        onOpenSession={onOpenSession}
        onRename={onRename}
        onDelete={onDelete}
        selection={selection}
        sessionIdCounts={sessionIdCounts}
      />
      {items.length > PAGE_SIZE && (
        <button type="button" className="sidebar-show-more" onClick={() => setLimit((current) => current === PAGE_SIZE ? items.length : PAGE_SIZE)}>
          {limit === PAGE_SIZE ? t("Show more", "显示更多") : t("Show less", "显示更少")}
        </button>
      )}
    </section>
  );
}
