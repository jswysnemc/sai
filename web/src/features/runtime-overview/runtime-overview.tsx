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
  Terminal
} from "lucide-react";
import { useEffect, useState } from "react";
import type { TodoStatus } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { OPEN_WORKSPACE_PANEL_EVENT } from "../workspace/workspace-panel-options";
import type { PaneTab } from "../workspace/workspace-tab";
import type { ActivityPulse } from "./activity-pulse";
import { useActivityPulse } from "./use-activity-pulse";
import { selectTodoOverviewItems, useRuntimeOverviewData } from "./runtime-overview-data";
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

  /** 胶囊内容：有活动播报时临时顶替常态的 Git 改动统计 */
  const pillContent = pulse
    ? <PulseContent pulse={pulse} />
    : (
      <>
        <FileDiff size={14} aria-hidden />
        <strong>{t("Changes", "更改")}</strong>
        <ChangeStats added={data.git.added} removed={data.git.removed} />
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
            <Button className="runtime-overview-agent" key={subagent.id} onClick={() => openWorkspacePanel("subagents")}>
              <span className={`runtime-overview-agent-state is-${subagent.status}`} aria-hidden />
              <span>{subagent.description || subagent.subagent_type}</span>
              <small>{subagentStatusLabel(subagent.status, locale)}</small>
            </Button>
          ))}
        </section>
      </div>
    </aside>
  );
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
