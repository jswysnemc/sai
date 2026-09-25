import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Button } from "../../shared/ui/button/button";
import { ContextActionMenu } from "../../shared/ui/menu/context-action-menu";
import { useI18n } from "../i18n/use-i18n";
import type { ImageWorkbenchSession } from "./image-workbench-store";

type ImageWorkbenchSessionsProps = {
  sessions: ImageWorkbenchSession[];
  activeId: string;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onRemove: (id: string) => void;
  onRemoveMany: (ids: string[]) => void;
};

type SessionMenu = { id: string; title: string; x: number; y: number };

/**
 * 渲染生图会话列表，支持右键菜单和多选删除。
 *
 * @param props 会话、当前项和增删回调
 * @returns 会话栏
 */
export function ImageWorkbenchSessions({ sessions, activeId, onSelect, onCreate, onRemove, onRemoveMany }: ImageWorkbenchSessionsProps) {
  const { t } = useI18n();
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set());
  const [menu, setMenu] = useState<SessionMenu | null>(null);
  const ids = sessions.map((session) => session.id);
  const allSelected = ids.length > 0 && ids.every((id) => selected.has(id));

  /**
   * 退出多选并清空已选项。
   */
  const exitSelection = () => {
    setSelecting(false);
    setSelected(new Set());
  };

  /**
   * 进入多选，并先勾上右键的那一条。
   *
   * @param id 要预先选中的会话
   */
  const enterSelection = (id: string) => {
    setSelecting(true);
    setSelected(new Set([id]));
  };

  /**
   * 切换一条会话的选中状态。
   *
   * @param id 会话标识
   */
  const toggle = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <aside className="image-sessions" aria-label={t("Image sessions", "生图会话")}>
      {selecting ? (
        <div className="image-sessions-head image-sessions-select">
          <button type="button" onClick={() => setSelected(allSelected ? new Set() : new Set(ids))}>{allSelected ? t("Clear", "取消") : t("All", "全选")}</button>
          <span>{t(`${selected.size} selected`, `已选 ${selected.size}`)}</span>
          <button type="button" className="is-danger" disabled={selected.size === 0} onClick={() => { onRemoveMany([...selected]); exitSelection(); }} aria-label={t("Delete selected sessions", "删除所选会话")}>
            <Trash2 size={13} />
          </button>
          <button type="button" onClick={exitSelection} aria-label={t("Exit selection", "退出选择")}>×</button>
        </div>
      ) : (
        <div className="image-sessions-head">
          <strong>{t("Sessions", "会话")}</strong>
          <Button variant="ghost" size="icon" onClick={onCreate} aria-label={t("New image session", "新建生图会话")} title={t("New image session", "新建生图会话")}><Plus size={14} /></Button>
        </div>
      )}
      <ul>
        {sessions.map((session) => {
          const checked = selected.has(session.id);
          return (
            <li key={session.id} className={session.id === activeId && !selecting ? "is-active" : checked ? "is-selected" : ""}>
              <button
                type="button"
                onClick={() => selecting ? toggle(session.id) : onSelect(session.id)}
                onContextMenu={(event) => {
                  event.preventDefault();
                  setMenu({ id: session.id, title: session.title, x: event.clientX, y: event.clientY });
                }}
                aria-current={!selecting && session.id === activeId ? "page" : undefined}
                aria-pressed={selecting ? checked : undefined}
              >
                {selecting && <span className={checked ? "image-session-check is-on" : "image-session-check"} aria-hidden />}
                <span>{session.title}</span>
                {session.turns.length > 0 && <small>{session.turns.length}</small>}
              </button>
              {!selecting && (
                <Button variant="ghost" size="icon" aria-label={t(`Delete ${session.title}`, `删除${session.title}`)} title={t("Delete", "删除")} onClick={() => onRemove(session.id)}><Trash2 size={13} /></Button>
              )}
            </li>
          );
        })}
      </ul>
      {menu && (
        <ContextActionMenu
          label={t("Session actions", "会话操作")}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { id: "select", label: selecting ? t("Exit selection", "退出选择") : t("Select sessions", "多选会话"), onSelect: () => selecting ? exitSelection() : enterSelection(menu.id) },
            { id: "delete", label: t("Delete", "删除"), danger: true, separator: true, onSelect: () => onRemove(menu.id) }
          ]}
        />
      )}
    </aside>
  );
}
