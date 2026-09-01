import { RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { BackgroundTaskCard } from "./background-task-card";
import { combineBackgroundTaskOutput, backgroundTaskStatusLabel, isBackgroundTaskRunning } from "./background-task-utils";
import { useBackgroundTasks } from "./use-background-tasks";
import "./background-tasks.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 后台任务面板。
 *
 * 空间跟随价值：实时输出是打开本页的主要目的，右侧输出区占主要空间；
 * 任务列表只承担导航，压缩为左侧紧凑列，运行中的任务排最前并保持选中；
 * 刷新与清理是低频操作，收在头部；cwd、PID 等元信息下沉到详情页头。
 */
export function BackgroundTasksPanel() {
  const { t, locale } = useI18n();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const manager = useBackgroundTasks(selectedId);

  // 运行中的任务最值得关注，排最前；其余按启动时间倒序
  const tasks = useMemo(() => {
    const running = manager.tasks.filter(isBackgroundTaskRunning);
    const rest = manager.tasks
      .filter((task) => !isBackgroundTaskRunning(task))
      .sort((left, right) => right.started_at - left.started_at);
    return [...running, ...rest];
  }, [manager.tasks]);

  // 选中失效时优先落到第一个运行中的任务
  useEffect(() => {
    if (selectedId && tasks.some((task) => task.id === selectedId)) return;
    setSelectedId(tasks.find(isBackgroundTaskRunning)?.id ?? tasks[0]?.id ?? null);
  }, [tasks, selectedId]);

  const selected = manager.tasks.find((task) => task.id === selectedId);
  const output = combineBackgroundTaskOutput(manager.output?.stdout, manager.output?.stderr);
  const runningCount = tasks.filter(isBackgroundTaskRunning).length;

  return (
    <section className="background-tasks-panel">
      <header>
        <div>
          <strong>{t("Background tasks", "后台任务")}</strong>
          {tasks.length > 0 && (
            <span>
              {runningCount > 0
                ? t(`${runningCount} running · ${tasks.length - runningCount} finished`, `${runningCount} 运行中 · ${tasks.length - runningCount} 已结束`)
                : t(`${tasks.length} tasks`, `${tasks.length} 个任务`)}
            </span>
          )}
        </div>
        <div className="background-task-actions">
          <button type="button" onClick={() => void manager.refresh()}><RefreshCw size={13} /><span>{t("Refresh", "刷新")}</span></button>
          <button type="button" onClick={() => void manager.cleanup()}><Trash2 size={13} /><span>{t("Clean finished", "清理已结束")}</span></button>
        </div>
      </header>
      {tasks.length === 0 && !manager.loading ? (
        <div className="background-task-empty-state">
          <p>{t("No background tasks", "没有后台任务")}</p>
          <small>{t("Commands started in the background appear here.", "后台启动的命令会出现在这里。")}</small>
        </div>
      ) : (
        <div className="background-task-layout">
          <div className="background-task-list">
            {tasks.map((task) => <BackgroundTaskCard key={task.id} task={task} active={task.id === selectedId} onSelect={() => setSelectedId(task.id)} onStop={() => void manager.stop(task.id)} />)}
          </div>
          <div className="background-task-detail">
            {selected ? (
              <>
                <header>
                  <div className="background-task-detail-title">
                    <strong>{selected.label}</strong>
                    <span className={`background-task-status ${selected.status}`}>{backgroundTaskStatusLabel(selected.status, locale)}</span>
                  </div>
                  <code>{selected.command}</code>
                  <small>
                    PID {selected.pid} · {selected.cwd}
                  </small>
                </header>
                <pre>{output || t("No output", "暂无输出")}</pre>
              </>
            ) : (
              <p className="background-task-detail-empty">{t("Select a task to view output", "选择任务查看输出")}</p>
            )}
          </div>
        </div>
      )}
      {manager.error && <div className="pane-error background-task-error">{manager.error.message}</div>}
    </section>
  );
}
