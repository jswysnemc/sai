import {
  Bot,
  CheckCircle2,
  ChevronRight,
  Circle,
  CircleDot,
  FileDiff,
  GitBranch,
  ListChecks,
  Minimize2
} from "lucide-react";
import { useEffect, useState } from "react";
import type { TodoStatus } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { OPEN_WORKSPACE_PANEL_EVENT } from "../workspace/workspace-panel-options";
import type { PaneTab } from "../workspace/workspace-tab";
import { useRuntimeOverviewData } from "./runtime-overview-data";
import "./runtime-overview.css";

const COLLAPSED_STORAGE_KEY = "sai.runtime-overview.collapsed";

const todoIcons = {
  pending: Circle,
  in_progress: CircleDot,
  completed: CheckCircle2,
  cancelled: Circle
} satisfies Record<TodoStatus, typeof Circle>;

type RuntimeOverviewProps = {
  sessionId?: string;
};

/**
 * 在聊天区右上角渲染 Git、Todo 和子智能体运行总览。
 *
 * @param props 当前会话标识
 * @returns 可收缩的运行总览浮层
 */
export function RuntimeOverview({ sessionId }: RuntimeOverviewProps) {
  const { t } = useI18n();
  const data = useRuntimeOverviewData(sessionId);
  const [responsiveOpen, setResponsiveOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === "undefined") return false;
    if (window.matchMedia("(max-width: 48rem)").matches) return true;
    return window.localStorage.getItem(COLLAPSED_STORAGE_KEY) === "true";
  });

  useEffect(() => {
    window.localStorage.setItem(COLLAPSED_STORAGE_KEY, String(collapsed));
  }, [collapsed]);

  if (collapsed) {
    return (
      <aside className="runtime-overview is-collapsed">
        <Button
          className="runtime-overview-pill"
          onClick={() => {
            setResponsiveOpen(true);
            setCollapsed(false);
          }}
          aria-label={t("Expand work overview", "展开工作概览")}
          title={t("Expand work overview", "展开工作概览")}
        >
          <FileDiff size={14} aria-hidden />
          <strong>{t("Changes", "更改")}</strong>
          <ChangeStats added={data.git.added} removed={data.git.removed} />
        </Button>
      </aside>
    );
  }

  return (
    <aside className={`runtime-overview is-expanded${responsiveOpen ? " is-responsive-open" : ""}`} aria-label={t("Work overview", "工作概览")}>
      <Button
        className="runtime-overview-pill runtime-overview-responsive-pill"
        onClick={() => setResponsiveOpen(true)}
        aria-label={t("Expand work overview", "展开工作概览")}
        title={t("Expand work overview", "展开工作概览")}
      >
        <FileDiff size={14} aria-hidden />
        <strong>{t("Changes", "更改")}</strong>
        <ChangeStats added={data.git.added} removed={data.git.removed} />
      </Button>
      <div className="runtime-overview-panel">
        <header className="runtime-overview-head">
          <strong>{t("Work overview", "工作概览")}</strong>
          <Button
            className="runtime-overview-collapse"
            onClick={() => {
              setResponsiveOpen(false);
              setCollapsed(true);
            }}
            aria-label={t("Collapse work overview", "收起工作概览")}
            title={t("Collapse work overview", "收起工作概览")}
          >
            <Minimize2 size={14} />
          </Button>
        </header>

        <section className="runtime-overview-section">
          <Button className="runtime-overview-row git-row" onClick={() => openWorkspacePanel("diff")}>
            <FileDiff size={14} aria-hidden />
            <strong>{t("Changes", "更改")}</strong>
            <span className="runtime-overview-count">{data.git.changedCount}</span>
            <ChangeStats added={data.git.added} removed={data.git.removed} />
          </Button>
          {data.git.branch && (
            <Button className="runtime-overview-row branch-row" onClick={() => openWorkspacePanel("diff")}>
              <GitBranch size={14} aria-hidden />
              <span>{data.git.branch}</span>
              <ChevronRight size={13} aria-hidden />
            </Button>
          )}
        </section>

        <section className="runtime-overview-section">
          <div className="runtime-overview-section-title">
            <ListChecks size={14} aria-hidden />
            <span>{t("Plan", "计划")}</span>
            <small>{data.todos.completed}/{data.todos.items.length}</small>
          </div>
          <div className="runtime-overview-list">
            {data.todos.items.slice(0, 4).map((item) => {
              const Icon = todoIcons[item.status];
              return (
                <div className={`runtime-overview-item is-${item.status}`} key={item.id}>
                  <Icon size={13} aria-hidden />
                  <span>{item.text}</span>
                </div>
              );
            })}
            {!data.todos.loading && data.todos.items.length === 0 && (
              <div className="runtime-overview-empty">{t("No active plan", "暂无计划")}</div>
            )}
          </div>
        </section>

        <section className="runtime-overview-section">
          <Button className="runtime-overview-section-title is-button" onClick={() => openWorkspacePanel("subagents")}>
            <Bot size={14} aria-hidden />
            <span>{t("Subagents", "子智能体")}</span>
            <small>{data.subagents.running > 0
              ? t(`${data.subagents.running} running`, `${data.subagents.running} 运行中`)
              : t(`${data.subagents.completed} completed`, `${data.subagents.completed} 已结束`)}</small>
            <ChevronRight size={13} aria-hidden />
          </Button>
          {data.subagents.items.slice(0, 3).map((subagent) => (
            <Button className="runtime-overview-agent" key={subagent.id} onClick={() => openWorkspacePanel("subagents")}>
              <span className={`runtime-overview-agent-state is-${subagent.status}`} aria-hidden />
              <span>{subagent.description || subagent.subagent_type}</span>
              <small>{subagent.status === "running" ? `${subagent.step}/${subagent.max_steps}` : subagent.status}</small>
            </Button>
          ))}
        </section>
      </div>
    </aside>
  );
}

/**
 * 渲染 Git 新增与删除行数。
 *
 * @param props 新增和删除行数
 * @returns 双统计文本
 */
function ChangeStats({ added, removed }: { added: number; removed: number }) {
  return (
    <span className="runtime-overview-change-stats" aria-label={`+${added} -${removed}`}>
      <b>+{added}</b>
      <i>-{removed}</i>
    </span>
  );
}

/**
 * 通知工作台打开指定面板。
 *
 * @param tab 目标工作区面板
 * @returns 无返回值
 */
function openWorkspacePanel(tab: PaneTab): void {
  window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab } }));
}
