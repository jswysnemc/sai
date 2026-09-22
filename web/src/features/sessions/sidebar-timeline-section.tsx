import { useState } from "react";
import { useI18n } from "../i18n/use-i18n";
import { SidebarFlatSessions } from "./sidebar-flat-sessions";
import { bucketSessions, type SidebarSessionRef } from "./sidebar-session-model";
import type { useSessionSelection } from "./use-session-selection";

type SelectionState = ReturnType<typeof useSessionSelection>;

const PAGE_SIZE = 20;

type SidebarTimelineSectionProps = {
  items: SidebarSessionRef[];
  runningSessions: ReadonlySet<string>;
  now: number;
  onOpenSession: (workspaceId: string, sessionId: string, workspaceActive: boolean, sessionActive: boolean) => void;
  onRename: (id: string, title: string) => Promise<void>;
  onDelete: (id: string, title: string) => void;
  selection: SelectionState;
};

/**
 * 按今天、昨天和更早的日期桶渲染会话。
 *
 * @param props 会话和时间基准
 * @returns 时间线
 */
export function SidebarTimelineSection({ items, runningSessions, now, onOpenSession, onRename, onDelete, selection }: SidebarTimelineSectionProps) {
  const { t } = useI18n();
  const [limit, setLimit] = useState(PAGE_SIZE);
  const visible = items.slice(0, limit);
  const buckets = bucketSessions(visible, now);
  if (!items.length) return <p className="session-list-empty">{t("No tasks yet", "还没有任务")}</p>;
  return (
    <div className="sidebar-timeline">
      {buckets.map((bucket) => (
        <section key={bucket.id}>
          <h3>{t(bucket.labelEn, bucket.labelZh)}</h3>
          <SidebarFlatSessions items={bucket.items} runningSessions={runningSessions} now={now} empty="" onOpenSession={onOpenSession} onRename={onRename} onDelete={onDelete} selection={selection} />
        </section>
      ))}
      {items.length > limit && (
        <button type="button" className="sidebar-show-more" onClick={() => setLimit((current) => current + PAGE_SIZE)}>{t("Show more", "显示更多")}</button>
      )}
    </div>
  );
}
