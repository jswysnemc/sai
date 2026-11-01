import type { BackgroundTask, BackgroundTaskOutput } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { FileText } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";
import { backgroundTaskLogLines, type BackgroundTaskStream } from "./background-task-log-model";

type BackgroundTaskDetailProps = {
  task: BackgroundTask;
  output?: BackgroundTaskOutput;
  loading: boolean;
  error: Error | null;
  stream: BackgroundTaskStream;
  onLogs: () => void;
};

/**
 * 展示选中任务元信息和当前通道末行摘要，将长日志留给独立日志模式。
 * @param props 任务、日志快照、查询状态、通道和日志打开回调
 * @returns 紧凑内联详情
 */
export function BackgroundTaskDetail({ task, output, loading, error, stream, onLogs }: BackgroundTaskDetailProps) {
  const { t } = useI18n();
  const last = backgroundTaskLogLines(output, stream).at(-1);
  return <section className="background-task-detail" aria-label={t("Task details", "任务详情")}>
    <header><span>{t("Selected task", "选中详情")}</span><Button variant="ghost" size="small" onClick={onLogs}><FileText size={14} />{t("View logs", "查看日志")}</Button></header>
    <dl className="background-task-metadata">
      <div><dt>ID</dt><dd>{task.id}</dd></div>
      <div><dt>PID</dt><dd>{task.pid}</dd></div>
      <div><dt>{t("Timeout", "超时")}</dt><dd>{task.timeout_seconds > 0 ? `${task.timeout_seconds}s` : t("Unlimited", "无限制")}</dd></div>
      <div className="background-task-path"><dt>{t("Directory", "目录")}</dt><dd title={task.cwd}>{task.cwd}</dd></div>
    </dl>
    <code className="background-task-command">{task.command}</code>
    <div className="background-task-preview" aria-busy={loading}>
      <span>{last?.stream.toUpperCase() ?? stream.toUpperCase()}</span>
      {error ? <p role="alert">{t("Could not read output: ", "读取输出失败：")}{error.message}</p>
        : <p title={last?.text}>{loading ? t("Loading output…", "正在读取输出…") : last?.text || t("No output", "暂无输出")}</p>}
    </div>
  </section>;
}
