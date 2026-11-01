import { Activity, RefreshCw, Square, Trash2, X } from "../../shared/ui/icons";
import { useEffect, useRef, useState } from "react";
import { Button } from "../../shared/ui/button/button";
import { Toast } from "../../shared/ui/notify/notify";
import { BackgroundTaskCard } from "./background-task-card";
import { BackgroundTaskDetail } from "./background-task-detail";
import { BackgroundTaskFilters } from "./background-task-filters";
import { BackgroundTaskLogView } from "./background-task-log-view";
import { isBackgroundTaskRunning, type BackgroundTaskFilter } from "./background-task-utils";
import type { BackgroundTaskStream } from "./background-task-log-model";
import { useBackgroundTaskActions } from "./use-background-task-actions";
import { useBackgroundTasks } from "./use-background-tasks";
import { useBackgroundTaskKeyboard } from "./use-background-task-keyboard";
import "./background-tasks.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 组合紧凑任务面板与原地日志查看器，共享真实任务状态和操作确认流程。
 * @returns 后台任务管理侧面板
 */
export function BackgroundTasksPanel() {
  const { t } = useI18n();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filter, setFilter] = useState<BackgroundTaskFilter>("all");
  const [logs, setLogs] = useState(false);
  const [stream, setStream] = useState<BackgroundTaskStream>("stdout");
  const listRef = useRef<HTMLDivElement>(null);
  const previousLogs = useRef(false);
  const manager = useBackgroundTasks(selectedId, filter);
  const actions = useBackgroundTaskActions(manager);
  const { tasks, visibleTasks, selected } = manager;
  const runningCount = tasks.filter(isBackgroundTaskRunning).length;
  const busy = Boolean(manager.stoppingId) || manager.cleaning;
  const canStop = Boolean(selected && isBackgroundTaskRunning(selected)) && !busy;

  // 1. 【后台任务】【选择保持】新增条目不打断当前输出阅读，失效选择由查询层修复
  useEffect(() => { setSelectedId(selected?.id ?? null); if (!selected) setLogs(false); }, [selected?.id]);
  useEffect(() => {
    const row = listRef.current?.querySelector<HTMLButtonElement>('[aria-pressed="true"]');
    if (!logs) row?.scrollIntoView({ block: "nearest" });
    if (!logs && previousLogs.current) row?.focus();
    previousLogs.current = logs;
  }, [selected?.id, logs]);

  /** 打开当前任务的日志模式，返回无值。 */
  const openLogs = () => { if (selected) setLogs(true); };
  /** 停止当前运行任务，force 决定是否强制停止，返回无值。 */
  const stop = (force: boolean) => { if (selected && canStop) void actions.stop(selected, force); };
  /** 清理已结束任务，无可清理记录时不提交请求，返回无值。 */
  const cleanup = () => { if (!busy && tasks.length > runningCount) void actions.cleanup(); };
  const keyboard = useBackgroundTaskKeyboard({ tasks: visibleTasks, selected, busy, onSelect: setSelectedId, onFilter: setFilter,
    onLogs: openLogs, onStop: stop, onCleanup: cleanup, onRefresh: () => void actions.refresh() });

  return <section className="background-tasks-panel @container" aria-label={t("Background tasks", "后台任务")}>
    <div className={`background-task-manager${logs && selected ? " log-mode" : ""}`} onKeyDown={logs ? undefined : keyboard}>
      <header className="background-task-heading">
        <Activity size={14} /><strong>{t("Background tasks", "后台任务管理")}</strong>
        <span>{runningCount}/{tasks.length} {t("running", "运行中")}</span>
        <Button variant="ghost" size="icon" onClick={() => void actions.refresh()} disabled={busy || manager.refreshing} aria-label={t("Refresh tasks", "刷新任务")} title={t("Refresh tasks", "刷新任务")}><RefreshCw size={14} /></Button>
      </header>
      {manager.error && <p className="background-task-inline-error" role="alert">{t("Could not load tasks: ", "获取任务失败：")}{manager.error.message}</p>}
      {logs && selected ? <BackgroundTaskLogView key={selected.id} task={selected} output={manager.output} loading={manager.outputLoading}
        error={manager.outputError} stream={stream} onStreamChange={setStream} onBack={() => setLogs(false)} /> : <>
        <BackgroundTaskFilters tasks={tasks} filter={filter} onChange={setFilter} />
        {manager.loading ? <div className="background-task-empty-state" role="status">{t("Loading tasks…", "正在获取任务…")}</div>
          : visibleTasks.length === 0 ? <div className="background-task-empty-state">
            <p>{manager.error ? t("Tasks unavailable", "暂时无法获取任务") : tasks.length ? t("No tasks match this filter", "当前筛选下没有任务") : t("No background tasks", "没有后台任务")}</p>
            <small>{manager.error ? t("Use Refresh to try again.", "请点击刷新重试。") : t("Commands started in the background appear here.", "后台启动的命令会出现在这里。")}</small>
          </div> : <div className="background-task-list" role="group" aria-label={t("Task list", "任务列表")} ref={listRef}>
            {visibleTasks.map((task) => <BackgroundTaskCard key={task.id} task={task} active={task.id === selected?.id} stopping={manager.stoppingId === task.id}
              onSelect={() => setSelectedId(task.id)} onLogs={() => { setSelectedId(task.id); setLogs(true); }} />)}
          </div>}
        {selected && <BackgroundTaskDetail task={selected} output={manager.output} loading={manager.outputLoading} error={manager.outputError} stream={stream} onLogs={openLogs} />}
        <footer className="background-task-actions">
          <Button variant="ghost-danger" size="small" onClick={() => stop(false)} disabled={!canStop} aria-busy={Boolean(manager.stoppingId)} aria-label={t("Stop task", "停止任务")}><Square size={12} />{manager.stoppingId ? t("Stopping", "正在停止") : t("Stop", "停止")}</Button>
          <Button variant="ghost-danger" size="small" onClick={() => stop(true)} disabled={!canStop}><X size={12} />{t("Force stop", "强制停止")}</Button>
          <Button variant="ghost" size="small" onClick={cleanup} disabled={busy || tasks.length === runningCount} aria-busy={manager.cleaning} aria-label={t("Clean finished tasks", "清理已结束任务")}><Trash2 size={12} />{t("Clean finished", "清理已结束")}</Button>
        </footer>
        <p className="background-task-key-hint hidden sm:block">{t("↑/↓ Select · Enter Logs · 1/2/3 Filter · s/f Stop · x Clean · r Refresh", "↑/↓ 选择 · Enter 日志 · 1/2/3 筛选 · s/f 停止 · x 清理 · r 刷新")}</p>
      </>}
    </div>
    <Toast notice={actions.notice} onDismiss={actions.dismissToast} />
  </section>;
}
