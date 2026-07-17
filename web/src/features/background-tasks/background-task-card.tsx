import { Square } from "lucide-react";
import type { BackgroundTask } from "../../api/contracts";
import { backgroundTaskStatusLabel, formatBackgroundTaskDuration, isBackgroundTaskRunning } from "./background-task-utils";
import { useI18n } from "../i18n/use-i18n";

/** 渲染单个后台任务摘要和停止操作。 */
export function BackgroundTaskCard({ task, active, onSelect, onStop }: { task: BackgroundTask; active: boolean; onSelect: () => void; onStop: () => void }) {
  const { locale, t } = useI18n();
  return (
    <article className={`background-task-card${active ? " active" : ""}`}>
      <button type="button" className="background-task-select" onClick={onSelect}>
        <span className={`background-task-status ${task.status}`}>{backgroundTaskStatusLabel(task.status, locale)}</span>
        <strong>{task.label}</strong>
        <code>{task.command}</code>
        <span>{task.cwd}</span>
        <small>PID {task.pid} · {formatBackgroundTaskDuration(task, undefined, locale)}</small>
      </button>
      {isBackgroundTaskRunning(task) && <button type="button" className="background-task-stop" onClick={onStop} aria-label={t(`Stop ${task.label}`, `停止 ${task.label}`)}><Square size={11} /></button>}
    </article>
  );
}
