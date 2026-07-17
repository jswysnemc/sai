import { useQuery } from "@tanstack/react-query";
import { Ban, CheckCircle2, ChevronDown, Circle, CircleDot, ListChecks } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../../api/client";
import type { TodoStatus } from "../../api/contracts";
import { useAnchoredPopover } from "../../shared/ui/popover/use-anchored-popover";
import { summarizeTodos } from "./todo-summary";
import "./todo-markdown.css";
import { useI18n } from "../i18n/use-i18n";

const statusIcons = {
  pending: Circle,
  in_progress: CircleDot,
  completed: CheckCircle2,
  cancelled: Ban
} satisfies Record<TodoStatus, typeof Circle>;

/**
 * 渲染 Agent 管理的只读 TODO 进度。
 *
 * 紧凑模式下弹层通过 Portal 挂到 body，避免被输入区 overflow 裁切。
 * 计划全部完成后前端不再展示；后端也会归档并清空活动列表。
 *
 * @param props sessionId 为当前会话，compact 为输入区紧凑样式
 * @returns TODO 进度触发器与清单
 */
export function TodoMarkdownView({ sessionId, compact = false }: { sessionId?: string; compact?: boolean }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const menuStyle = useAnchoredPopover({
    open: compact && open,
    anchorRef: triggerRef,
    preferredWidth: 300,
    minimumWidth: 240,
    align: "right",
    maxHeight: 300
  });
  const query = useQuery({
    queryKey: ["todos", sessionId],
    queryFn: api.todos.list,
    enabled: Boolean(sessionId),
    refetchInterval: 2000
  });

  useEffect(() => {
    if (!(compact && open)) return;

    /** 在触发器和弹层外按下指针时关闭清单。 */
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || listRef.current?.contains(target)) return;
      setOpen(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [compact, open]);

  if (!sessionId || !query.data?.length) return null;

  const items = query.data;
  const summary = summarizeTodos(items);
  // 全部完成的计划由后端归档后列表为空；这里再兜底隐藏已完成清单。
  if (summary.allDone) return null;
  const percent = Math.round(summary.ratio * 100);
  const list = open ? (
    <ul
      ref={listRef}
      className={compact ? "todo-markdown-list todo-markdown-popover" : "todo-markdown-list"}
      style={compact ? menuStyle : undefined}
      role="listbox"
      aria-label={t("Plan checklist", "计划清单")}
    >
      {items.map((item) => {
        const Icon = statusIcons[item.status];
        return (
          <li key={item.id} className={`todo-markdown-item is-${item.status}`}>
            <Icon size={15} />
            <span>{item.text}</span>
          </li>
        );
      })}
    </ul>
  ) : null;

  return (
    <section ref={rootRef} className={`todo-markdown-view${compact ? " compact" : ""}`}>
      <button
        ref={triggerRef}
        type="button"
        className="todo-markdown-trigger"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-haspopup="listbox"
      >
        <span className="todo-trigger-icon">
          <ListChecks size={compact ? 13 : 14} />
        </span>
        <span className="todo-trigger-body">
          <span className="todo-trigger-line">
            <strong>{summary.activeText || t("Plan", "计划")}</strong>
            <span className="todo-trigger-count">
              {summary.completed}/{summary.total}
            </span>
          </span>
          {!compact && (
            <span className="todo-trigger-track" aria-hidden>
              <span className="todo-trigger-fill" style={{ width: `${percent}%` }} />
            </span>
          )}
        </span>
        <ChevronDown size={compact ? 12 : 15} className={open ? "open" : ""} />
      </button>
      {compact && list ? createPortal(list, document.body) : list}
      {query.error && <div className="run-error">{query.error.message}</div>}
    </section>
  );
}
