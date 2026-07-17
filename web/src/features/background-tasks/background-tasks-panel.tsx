import { RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { BackgroundTaskCard } from "./background-task-card";
import { combineBackgroundTaskOutput } from "./background-task-utils";
import { useBackgroundTasks } from "./use-background-tasks";
import "./background-tasks.css";
import { useI18n } from "../i18n/use-i18n";

/** 渲染后台任务列表、任务详情和管理操作。 */
export function BackgroundTasksPanel() {
  const { t } = useI18n();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const manager = useBackgroundTasks(selectedId);
  useEffect(() => {
    if (!selectedId || !manager.tasks.some((task) => task.id === selectedId)) setSelectedId(manager.tasks[0]?.id ?? null);
  }, [manager.tasks, selectedId]);
  const selected = manager.tasks.find((task) => task.id === selectedId);
  const output = combineBackgroundTaskOutput(manager.output?.stdout, manager.output?.stderr);
  return (
    <section className="background-tasks-panel">
      <header>
        <div><strong>{t("Background tasks", "后台任务")}</strong><span>{t(`${manager.tasks.length} tasks`, `${manager.tasks.length} 个任务`)}</span></div>
        <div className="background-task-actions">
          <button type="button" onClick={() => void manager.refresh()}><RefreshCw size={13} /><span>{t("Refresh", "刷新")}</span></button>
          <button type="button" onClick={() => void manager.cleanup()}><Trash2 size={13} /><span>{t("Clean finished", "清理已结束")}</span></button>
        </div>
      </header>
      <div className="background-task-layout">
        <div className="background-task-list">
          {manager.tasks.map((task) => <BackgroundTaskCard key={task.id} task={task} active={task.id === selectedId} onSelect={() => setSelectedId(task.id)} onStop={() => void manager.stop(task.id)} />)}
          {!manager.loading && manager.tasks.length === 0 && <p className="background-task-empty">{t("No background tasks", "没有后台任务")}</p>}
        </div>
        <div className="background-task-detail">
          {selected ? <><header><strong>{selected.label}</strong><code>{selected.command}</code></header><pre>{output || t("No output", "暂无输出")}</pre></> : <p>{t("Select a task to view output", "选择任务查看输出")}</p>}
        </div>
      </div>
      {manager.error && <div className="pane-error background-task-error">{manager.error.message}</div>}
    </section>
  );
}
