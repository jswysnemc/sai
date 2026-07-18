import { useQuery } from "@tanstack/react-query";
import { Ban, CheckCircle2, ChevronDown, ChevronLeft, ChevronRight, Circle, CircleDot, ListChecks } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { api } from "../../api/client";
import type { TodoHistoryBatch, TodoItem, TodoStatus } from "../../api/contracts";
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

type PlanView = {
  key: string;
  label: string;
  items: TodoItem[];
  archived: boolean;
  archivedAt?: string;
};

/**
 * 渲染 Agent 管理的只读 TODO 进度，支持横向切换历史计划。
 *
 * 紧凑模式下弹层通过 Portal 挂到 body，避免被输入区 overflow 裁切。
 * 计划完成后仍保留展示，并通过轮询实时刷新状态。
 *
 * @param props sessionId 为当前会话，compact 为输入区紧凑样式
 * @returns TODO 进度触发器与清单
 */
export function TodoMarkdownView({ sessionId, compact = false }: { sessionId?: string; compact?: boolean }) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [planIndex, setPlanIndex] = useState(0);
  const rootRef = useRef<HTMLElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const menuStyle = useAnchoredPopover({
    open: compact && open,
    anchorRef: triggerRef,
    preferredWidth: 320,
    minimumWidth: 240,
    align: "right",
    maxHeight: 320
  });
  const query = useQuery({
    queryKey: ["todos", sessionId],
    queryFn: api.todos.list,
    enabled: Boolean(sessionId),
    // 运行中更快刷新状态
    refetchInterval: open ? 1000 : 1500
  });

  const plans = useMemo(
    () => buildPlans(query.data?.items ?? [], query.data?.history ?? [], t, locale),
    [query.data?.history, query.data?.items, locale, t]
  );

  // 新计划出现时回到最新活动计划
  useEffect(() => {
    setPlanIndex(0);
  }, [plans[0]?.key]);

  useEffect(() => {
    if (planIndex >= plans.length) setPlanIndex(Math.max(0, plans.length - 1));
  }, [planIndex, plans.length]);

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

  if (!sessionId || plans.length === 0) return null;

  const safeIndex = Math.min(planIndex, plans.length - 1);
  const plan = plans[safeIndex];
  const summary = summarizeTodos(plan.items);
  const percent = Math.round(summary.ratio * 100);
  const canPrev = safeIndex < plans.length - 1;
  const canNext = safeIndex > 0;

  const shiftPlan = (delta: number, event: MouseEvent) => {
    event.stopPropagation();
    setPlanIndex((value) => Math.min(plans.length - 1, Math.max(0, value + delta)));
  };

  const list = open ? (
    <ul
      ref={listRef}
      className={compact ? "todo-markdown-list todo-markdown-popover" : "todo-markdown-list"}
      style={compact ? menuStyle : undefined}
      role="listbox"
      aria-label={t("Plan checklist", "计划清单")}
    >
      {plans.length > 1 && (
        <li className="todo-plan-switcher" role="presentation">
          <button type="button" disabled={!canPrev} onClick={(event) => shiftPlan(1, event)} aria-label={t("Older plan", "更早计划")}>
            <ChevronLeft size={14} />
          </button>
          <span>
            {plan.label}
            <small>
              {safeIndex + 1}/{plans.length}
              {plan.archivedAt ? ` · ${formatArchiveTime(plan.archivedAt, locale)}` : ""}
            </small>
          </span>
          <button type="button" disabled={!canNext} onClick={(event) => shiftPlan(-1, event)} aria-label={t("Newer plan", "更新计划")}>
            <ChevronRight size={14} />
          </button>
        </li>
      )}
      {plan.items.map((item) => {
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
    <section ref={rootRef} className={`todo-markdown-view${compact ? " compact" : ""}${summary.allDone ? " is-done" : ""}`}>
      <div className="todo-markdown-bar">
        {plans.length > 1 && (
          <button
            type="button"
            className="todo-plan-nav"
            disabled={!canPrev}
            onClick={(event) => shiftPlan(1, event)}
            aria-label={t("Older plan", "更早计划")}
            title={t("Older plan", "更早计划")}
          >
            <ChevronLeft size={compact ? 13 : 15} />
          </button>
        )}
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
              <strong>{summary.activeText || plan.label || t("Plan", "计划")}</strong>
              <span className="todo-trigger-count">
                {summary.completed}/{summary.total}
                {summary.allDone ? ` · ${t("Done", "已完成")}` : ""}
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
        {plans.length > 1 && (
          <button
            type="button"
            className="todo-plan-nav"
            disabled={!canNext}
            onClick={(event) => shiftPlan(-1, event)}
            aria-label={t("Newer plan", "更新计划")}
            title={t("Newer plan", "更新计划")}
          >
            <ChevronRight size={compact ? 13 : 15} />
          </button>
        )}
      </div>
      {compact && list ? createPortal(list, document.body) : list}
      {query.error && <div className="run-error">{query.error.message}</div>}
    </section>
  );
}

/**
 * 组装可切换的计划视图：当前活动计划在前，历史从新到旧。
 */
function buildPlans(
  items: TodoItem[],
  history: TodoHistoryBatch[],
  t: (en: string, zh: string) => string,
  locale: string
): PlanView[] {
  const plans: PlanView[] = [];
  if (items.length > 0) {
    const summary = summarizeTodos(items);
    plans.push({
      key: `active:${items.map((item) => item.id).join(",")}`,
      label: summary.allDone ? t("Latest plan", "最近计划") : t("Current plan", "当前计划"),
      items,
      archived: false
    });
  }
  // history 文件从旧到新；界面从新到旧浏览
  for (let index = history.length - 1; index >= 0; index -= 1) {
    const batch = history[index];
    if (!batch.items.length) continue;
    plans.push({
      key: `history:${batch.archived_at}:${index}`,
      label: t("Archived plan", "历史计划"),
      items: batch.items,
      archived: true,
      archivedAt: batch.archived_at
    });
  }
  return plans;
}

/** 格式化归档时间。 */
function formatArchiveTime(value: string, locale: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale === "zh-CN" ? "zh-CN" : "en-US", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}
