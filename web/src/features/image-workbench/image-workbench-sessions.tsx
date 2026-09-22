import { Plus, Trash2 } from "lucide-react";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import type { ImageWorkbenchSession } from "./image-workbench-store";

type ImageWorkbenchSessionsProps = {
  sessions: ImageWorkbenchSession[];
  activeId: string;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onRemove: (id: string) => void;
};

/**
 * 渲染生图会话列表。
 *
 * @param props 会话、当前项和增删回调
 * @returns 会话栏
 */
export function ImageWorkbenchSessions({ sessions, activeId, onSelect, onCreate, onRemove }: ImageWorkbenchSessionsProps) {
  const { t } = useI18n();
  return (
    <aside className="image-sessions" aria-label={t("Image sessions", "生图会话")}>
      <div className="image-sessions-head">
        <strong>{t("Sessions", "会话")}</strong>
        <Button variant="ghost" size="icon" onClick={onCreate} aria-label={t("New image session", "新建生图会话")} title={t("New image session", "新建生图会话")}><Plus size={14} /></Button>
      </div>
      <ul>
        {sessions.map((session) => (
          <li key={session.id} className={session.id === activeId ? "is-active" : ""}>
            <button type="button" onClick={() => onSelect(session.id)} aria-current={session.id === activeId ? "page" : undefined}>
              <span>{session.title}</span>
              {session.turns.length > 0 && <small>{session.turns.length}</small>}
            </button>
            <Button variant="ghost" size="icon" aria-label={t(`Delete ${session.title}`, `删除${session.title}`)} title={t("Delete", "删除")} onClick={() => onRemove(session.id)}><Trash2 size={13} /></Button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
