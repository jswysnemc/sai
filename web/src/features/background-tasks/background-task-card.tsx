import { Square } from "lucide-react";
import type { BackgroundTask } from "../../api/contracts";
import { formatBackgroundTaskDuration, isBackgroundTaskRunning } from "./background-task-utils";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染单个后台任务的列表项。
 *
 * 卡片只承担导航：状态点 + 名称 + 命令一行；
 * cwd、PID 等元信息在详情页头展示，不在这里重复。
 */
export function BackgroundTaskCard({ task, active, onSelect, onStop }: { task: BackgroundTask; active: boolean; onSelect: () => void; onStop: () => void }) {
  const { locale, t } = useI18n();
  const running = isBackgroundTaskRunning(task);
  return (
    <article className={`background-task-card${active ? " active" : ""}${running ? " running" : ""}`}>
      <button type="button" className="background-task-select" onClick={onSelect}>
        <span className="background-task-dot" aria-hidden />
        <span className="background-task-copy">
          <strong>{task.label}</strong>
          <code>{task.command}</code>
        </span>
        <small>{running ? formatBackgroundTaskDuration(task, undefined, locale) : t("Done", "已结束")}</small>
      </button>
      {running && <button type="button" className="background-task-stop" onClick={onStop} aria-label={t(`Stop ${task.label}`, `停止 ${task.label}`)}><Square size={11} /></button>}
    </article>
  );
}
