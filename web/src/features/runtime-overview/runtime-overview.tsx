import {
  Bot,
  CheckCircle2,
  ChevronRight,
  Circle,
  CircleDot,
  FileDiff,
  GitBranch,
  ListChecks,
  Minimize2,
  Target,
  Terminal
} from "lucide-react";
import { useEffect, useState } from "react";
import type { TodoStatus } from "../../api/contracts";
import type { Goal, GoalStatus } from "../../api/goal-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { OPEN_WORKSPACE_PANEL_EVENT } from "../workspace/workspace-panel-options";
import type { PaneTab } from "../workspace/workspace-tab";
import type { ActivityPulse } from "./activity-pulse";
import { useActivityPulse } from "./use-activity-pulse";
import { selectTodoOverviewItems, useRuntimeOverviewData } from "./runtime-overview-data";
import { requestSubagentFocus } from "../subagents/subagent-focus";
import { subagentStatusLabel } from "../subagents/subagent-labels";
import "./runtime-overview.css";

const COLLAPSED_STORAGE_KEY = "sai.runtime-overview.collapsed";

const todoIcons = {
  pending: Circle,
  in_progress: CircleDot,
  completed: CheckCircle2,
  cancelled: Circle
} satisfies Record<TodoStatus, typeof Circle>;

/** 各类活动播报对应的图标 */
const pulseIcons = {
  task: Terminal,
  todo: ListChecks,
  subagent: Bot
} satisfies Record<ActivityPulse["kind"], typeof Circle>;

type RuntimeOverviewProps = {
  sessionId?: string;
};

/**
 * 在聊天区右上角渲染 Git、Todo 和子智能体运行总览。
 *
 * 常态展示 Git 改动；后台命令启停、Todo 推进、子智能体启停时，
 * 胶囊会临时切换到对应播报，数秒后自动回到常态。
 *
 * @param props 当前会话标识
 * @returns 可收缩的运行总览浮层
 */
export function RuntimeOverview({ sessionId }: RuntimeOverviewProps) {
  const { locale, t } = useI18n();
  const data = useRuntimeOverviewData(sessionId);
  const pulse = useActivityPulse(data.snapshot);
  const [responsiveOpen, setResponsiveOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === "undefined") return false;
    if (window.matchMedia("(max-width: 48rem)").matches) return true;
    return window.localStorage.getItem(COLLAPSED_STORAGE_KEY) === "true";
  });

  useEffect(() => {
    window.localStorage.setItem(COLLAPSED_STORAGE_KEY, String(collapsed));
  }, [collapsed]);

  const activeTodo = data.todos.items.find((item) => item.status === "in_progress")
    ?? data.todos.items.find((item) => item.status === "pending");

  /** 胶囊内容：活动播报优先，其次展示 Todo 或子智能体，Git 不可用时不占位。 */
  const pillContent = pulse
    ? <PulseContent pulse={pulse} />
    : activeTodo
      ? (
        <>
          <ListChecks size={14} aria-hidden />
          <strong>{activeTodo.text}</strong>
          <span className="runtime-overview-pill-progress">{data.todos.completed}/{data.todos.items.length}</span>
        </>
      )
      : data.subagents.running > 0
        ? (
          <>
            <Bot size={14} aria-hidden />
            <strong>{t("Subagents", "子智能体")}</strong>
            <span className="runtime-overview-pill-progress">{data.subagents.running}</span>
          </>
        )
        : data.git.available
          ? (
            <>
              <FileDiff size={14} aria-hidden />
              <strong>{t("Changes", "更改")}</strong>
              <ChangeStats added={data.git.added} removed={data.git.removed} />
            </>
          )
          : (
            <>
              <ListChecks size={14} aria-hidden />
              <strong>{t("Plan", "计划")}</strong>
              <span className="runtime-overview-pill-progress">{data.todos.completed}/{data.todos.items.length}</span>
            </>
          );

  if (collapsed) {
    return (
      <aside className="runtime-overview is-collapsed">
        <Button
          className={`runtime-overview-pill${pulse ? " is-pulsing" : ""}`}
          onClick={() => {
            setResponsiveOpen(true);
            setCollapsed(false);
          }}
          aria-label={t("Expand work overview", "展开工作概览")}
          title={t("Expand work overview", "展开工作概览")}
        >
          {pillContent}
        </Button>
      </aside>
    );
  }

  return (
    <aside className={`runtime-overview is-expanded${responsiveOpen ? " is-responsive-open" : ""}`} aria-label={t("Work overview", "工作概览")}>
      <Button
        className={`runtime-overview-pill runtime-overview-responsive-pill${pulse ? " is-pulsing" : ""}`}
        onClick={() => setResponsiveOpen(true)}
        aria-label={t("Expand work overview", "展开工作概览")}
        title={t("Expand work overview", "展开工作概览")}
      >
        {pillContent}
      </Button>
      <div className="runtime-overview-panel">
        <header className="runtime-overview-head">
          <strong>{t("Work overview", "工作概览")}</strong>
          {pulse && <PulseContent pulse={pulse} className="runtime-overview-head-pulse" />}
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
          <div className="runtime-overview-section-title">
            <ListChecks size={14} aria-hidden />
            <span>{t("Plan", "计划")}</span>
            <small>{data.todos.completed}/{data.todos.items.length}</small>
          </div>
          <div className="runtime-overview-list">
            {selectTodoOverviewItems(data.todos.items).map((item) => {
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
          {data.subagents.overviewItems.map((subagent) => (
            <Button
              className="runtime-overview-agent"
              key={subagent.id}
              onClick={() => {
                // 条目直达对应详情，只打开面板等于让用户在列表里再找一遍
                requestSubagentFocus(subagent.id);
                openWorkspacePanel("subagents");
              }}
            >
              <span className={`runtime-overview-agent-state is-${subagent.status}`} aria-hidden />
              <span>{subagent.description || subagent.subagent_type}</span>
              <small>{subagentStatusLabel(subagent.status, locale)}</small>
            </Button>
          ))}
        </section>

        {data.git.available && (
          <section className="runtime-overview-section">
            <Button className="runtime-overview-row git-row" onClick={openGitFileTree}>
              <FileDiff size={14} aria-hidden />
              <strong>{t("Changes", "更改")}</strong>
              <span className="runtime-overview-count">{data.git.changedCount}</span>
              <ChangeStats added={data.git.added} removed={data.git.removed} />
            </Button>
            {data.git.branch && (
              <Button className="runtime-overview-row branch-row" onClick={openGitFileTree}>
                <GitBranch size={14} aria-hidden />
                <span>{data.git.branch}</span>
                <ChevronRight size={13} aria-hidden />
              </Button>
            )}
          </section>
        )}

        {data.goal.item && <GoalOverview goal={data.goal.item} />}
      </div>
    </aside>
  );
}

/**
 * 渲染当前会话目标的摘要、状态、用量和最近进度。
 *
 * @param props 当前目标
 * @returns 目标总览区域
 */
function GoalOverview({ goal }: { goal: Goal }) {
  const { t, locale } = useI18n();
  const objective = goal.objective
    .split("\\n")
    .map((line) => line.trim())
    .find((line) => line && !line.includes("data:image/"))
    ?? t("Untitled goal", "未命名目标");
  const updates = (goal.updates ?? []).slice(-5).reverse();
  const statusLabel = goalStatusLabel(goal.status, t);
  const timeLabel = formatGoalDuration(goal.time_used_seconds, locale);
  const budgetLabel = goal.token_budget == null
    ? t("Unlimited", "不限")
    : goal.token_budget.toLocaleString(locale.startsWith("zh") ? "zh-CN" : "en-US");

  return (
    <section className="runtime-overview-section runtime-overview-goal" aria-label={t("Goal", "目标")}>
      <div className="runtime-overview-section-title">
        <Target size={14} aria-hidden />
        <span>{t("Goal", "目标")}</span>
        <small>{statusLabel}</small>
      </div>
      <strong className="runtime-overview-goal-objective" title={objective}>{objective}</strong>
      <div className="runtime-overview-goal-meta">
        <span>{goal.tokens_used.toLocaleString(locale.startsWith("zh") ? "zh-CN" : "en-US")} / {budgetLabel} tokens</span>
        <span>{timeLabel}</span>
      </div>
      {updates.length > 0 && (
        <div className="runtime-overview-goal-progress">
          <span className="runtime-overview-goal-progress-label">{t("Progress", "进度")}</span>
          {updates.map((entry, index) => {
            const completed = entry.status === "complete" || entry.status === "completed";
            const current = entry.status === "active" || entry.status === "in_progress";
            const Icon = completed ? CheckCircle2 : current ? CircleDot : Circle;
            return (
              <div className={`runtime-overview-item runtime-overview-goal-update${completed ? " is-completed" : ""}`} key={`${entry.at}-${index}`}>
                <Icon size={13} aria-hidden />
                <span title={entry.message}>{entry.message}</span>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

/** 将目标状态映射为界面文案。 */
function goalStatusLabel(status: GoalStatus, t: (english: string, chinese: string) => string): string {
  const labels: Record<GoalStatus, [string, string]> = {
    active: ["Active", "进行中"],
    paused: ["Paused", "已暂停"],
    blocked: ["Blocked", "受阻"],
    usage_limited: ["Usage limited", "用量受限"],
    budget_limited: ["Budget reached", "预算已用尽"],
    complete: ["Complete", "已完成"]
  };
  return t(...labels[status]);
}

/** 将目标运行秒数格式化为紧凑的本地化时长。 */
function formatGoalDuration(seconds: number, locale: string): string {
  if (seconds < 60) return new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(seconds) + (locale.startsWith("zh") ? "秒" : "s");
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}${locale.startsWith("zh") ? "分" : "m"}`;
  return `${Math.floor(minutes / 60)}${locale.startsWith("zh") ? "时" : "h"} ${minutes % 60}${locale.startsWith("zh") ? "分" : "m"}`;
}

/**
 * 渲染一条活动播报的图标与文字。
 *
 * @param props 播报内容与附加类名
 * @returns 播报行
 */
function PulseContent({ pulse, className }: { pulse: ActivityPulse; className?: string }) {
  const Icon = pulseIcons[pulse.kind];
  return (
    <span className={`runtime-overview-pulse is-${pulse.kind}${className ? ` ${className}` : ""}`} role="status">
      <Icon size={14} aria-hidden />
      <span>{pulse.message}</span>
    </span>
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
 * @param revealFileTree 是否同时展开编辑器文件树
 * @returns 无返回值
 */
function openWorkspacePanel(tab: PaneTab, revealFileTree = false): void {
  window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, {
    detail: { tab, revealFileTree }
  }));
}

/**
 * 打开集成 Git 状态的文件树。
 *
 * 返回:
 * - 无返回值
 */
function openGitFileTree(): void {
  openWorkspacePanel("files", true);
}
