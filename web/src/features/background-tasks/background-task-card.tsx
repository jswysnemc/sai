import { Button } from "../../shared/ui/button/button";
import type { BackgroundTask } from "../../api/contracts";
import { backgroundTaskStatusBadge, backgroundTaskStatusLabel, formatBackgroundTaskDuration } from "./background-task-utils";
import { useI18n } from "../i18n/use-i18n";

type BackgroundTaskCardProps = {
  task: BackgroundTask;
  active: boolean;
  stopping: boolean;
  onSelect: () => void;
  onLogs: () => void;
};

/**
 * 渲染与终端演示一致的紧凑单行任务，双击或 Enter 查看日志。
 * @param props 任务、选中状态、停止状态和导航回调
 * @returns 状态、名称、PID 与耗时对齐的任务行
 */
export function BackgroundTaskCard({ task, active, stopping, onSelect, onLogs }: BackgroundTaskCardProps) {
  const { locale, t } = useI18n();
  const status = backgroundTaskStatusLabel(task.status, locale);
  return <Button variant="ghost" className="background-task-select" onClick={onSelect} onDoubleClick={onLogs} aria-pressed={active}
    title={`${task.label}\n${status}\n${task.command}`}>
    <span className={`background-task-status ${task.status}`} aria-label={status}>{backgroundTaskStatusBadge(task.status)}</span>
    <strong>{task.label || task.command}</strong>
    <span className="background-task-pid hidden @min-[28rem]:inline">PID {task.pid}</span>
    <small>{stopping ? t("Stopping", "正在停止") : formatBackgroundTaskDuration(task, undefined, locale)}</small>
  </Button>;
}
