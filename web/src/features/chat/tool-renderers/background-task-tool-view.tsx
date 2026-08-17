import { CollapsibleOutput } from "./collapsible-output";
import { ShellCommandLine } from "./shell-command-line";
import { parseJsonRecord, stringField, type JsonRecord } from "./tool-data";
import { useI18n } from "../../i18n/use-i18n";

type BackgroundTaskToolViewProps = {
  argumentsText: string;
  output: string;
};

type Translate = (en: string, zh: string) => string;

/**
 * 渲染后台任务管理调用的语义视图。
 *
 * background_command 只有 action=start 真的是一条 shell 命令；
 * list/output/wait/stop/cleanup 都是任务管理操作，套用终端渲染会把
 * `{"action":"output", ...}` 参数 JSON 当成命令、还会因为结果里没有
 * exit_code 而挂出"退出码 未知"的假失败。这里按 action 分别表达：
 * start 复用命令行样式并补上任务去向，list 渲染任务表，output 渲染
 * 日志流，wait/stop/cleanup 渲染操作结果。
 *
 * @param props 工具参数与结果
 * @returns 后台任务管理视图
 */
export function BackgroundTaskToolView({ argumentsText, output }: BackgroundTaskToolViewProps) {
  const { t } = useI18n();
  const args = parseJsonRecord(argumentsText);
  const result = parseJsonRecord(output);
  const action = stringField(args, "action") || "start";
  const command = stringField(args, "command");
  const task = recordField(result, "task");
  const taskId = stringField(args, "task_id") || stringField(task, "id") || stringField(result, "task_id");

  // 工具级失败（未知 action、任务不存在等）没有 JSON 结果，直接展示原始错误文本
  if (!result) {
    return output ? (
      <div className="bg-task-view">
        <CollapsibleOutput source={output} className="shell-output" />
      </div>
    ) : null;
  }

  return (
    <div className="bg-task-view">
      {command && <ShellCommandLine command={command} hasBody />}
      <div className="bg-task-meta">
        <span className="bg-task-action">{backgroundActionLabel(action, t)}</span>
        {taskId && <code className="bg-task-id">{taskId}</code>}
        {task && <TaskStatusMark status={stringField(task, "status")} t={t} />}
      </div>
      {action === "list" && <TaskList result={result} t={t} />}
      {action === "output" && <TaskLogs result={result} t={t} />}
      {action === "wait" && <WaitOutcome result={result} t={t} />}
      {action === "stop" && <StopOutcome result={result} t={t} />}
      {action === "cleanup" && <CleanupOutcome result={result} t={t} />}
    </div>
  );
}

/**
 * 返回后台任务 action 的双语可读标签。
 *
 * @param action 后台任务操作
 * @param t 双语文本选择方法
 * @returns 可读标签
 */
export function backgroundActionLabel(action: string, t: Translate): string {
  switch (action) {
    case "start":
      return t("Started background task", "已启动后台任务");
    case "list":
      return t("Task list", "任务列表");
    case "output":
      return t("Task output", "任务输出");
    case "wait":
      return t("Wait for task", "等待任务");
    case "stop":
      return t("Stop task", "停止任务");
    case "cleanup":
      return t("Clean up tasks", "清理任务");
    default:
      return action;
  }
}

/**
 * 渲染任务状态标记。
 *
 * @param props 状态字符串与双语文本选择方法
 * @returns 状态标记元素
 */
function TaskStatusMark({ status, t }: { status: string; t: Translate }) {
  const labels: Record<string, string> = {
    running: t("Running", "运行中"),
    exited: t("Exited", "已退出"),
    stopped: t("Stopped", "已停止"),
    timed_out: t("Timed out", "已超时")
  };
  const tone = status === "running" ? "is-running" : status === "stopped" || status === "timed_out" ? "is-warning" : "";
  return <span className={`bg-task-status ${tone}`.trim()}>{labels[status] ?? status}</span>;
}

/**
 * 渲染 action=list 的任务表。
 *
 * @param props 结果对象与双语文本选择方法
 * @returns 任务行列表或空态
 */
function TaskList({ result, t }: { result: JsonRecord; t: Translate }) {
  const tasks = Array.isArray(result.tasks) ? result.tasks.filter(isRecord) : [];
  if (tasks.length === 0) {
    return <div className="bg-task-empty">{t("No background tasks", "没有后台任务")}</div>;
  }
  return (
    <ul className="bg-task-list">
      {tasks.map((task) => {
        const status = stringField(task, "status");
        const name = stringField(task, "label") || stringField(task, "command");
        return (
          <li key={stringField(task, "id")}>
            <i className={`bg-task-dot is-${status || "unknown"}`} aria-hidden />
            <span className="bg-task-name" title={name}>{name}</span>
            <code className="bg-task-id">{stringField(task, "id")}</code>
            <TaskStatusMark status={status} t={t} />
          </li>
        );
      })}
    </ul>
  );
}

/**
 * 渲染 action=output 的日志流。
 *
 * @param props 结果对象与双语文本选择方法
 * @returns stdout/stderr 输出块
 */
function TaskLogs({ result, t }: { result: JsonRecord; t: Translate }) {
  const stdout = stringField(result, "stdout");
  const stderr = stringField(result, "stderr");
  const truncated = result.stdout_truncated === true || result.stderr_truncated === true;
  if (!stdout && !stderr) {
    return <div className="bg-task-empty">{t("The task has no output yet", "该任务还没有输出")}</div>;
  }
  return (
    <>
      {stdout && <CollapsibleOutput source={stdout} />}
      {stderr && <CollapsibleOutput source={stderr} className="shell-output stderr" />}
      {truncated && <div className="bg-task-note">{t("Output is truncated; read again with head_lines/tail_lines", "输出已截断，可用 head_lines/tail_lines 再次读取")}</div>}
    </>
  );
}

/**
 * 渲染 action=wait 的等待结果。
 *
 * @param props 结果对象与双语文本选择方法
 * @returns 等待结论行
 */
function WaitOutcome({ result, t }: { result: JsonRecord; t: Translate }) {
  if (result.timeout === true) {
    return <div className="bg-task-note">{t("Wait timed out; the task is still running", "等待超时，任务仍在运行")}</div>;
  }
  if (result.completed === false) {
    return <div className="bg-task-empty">{t("No running background tasks to wait for", "没有正在运行的后台任务")}</div>;
  }
  return null;
}

/**
 * 渲染 action=stop 的停止结果。
 *
 * @param props 结果对象与双语文本选择方法
 * @returns 停止结论行
 */
function StopOutcome({ result, t }: { result: JsonRecord; t: Translate }) {
  return (
    <div className="bg-task-note">
      {result.was_running === true
        ? t("Stop signal sent", "已发送停止信号")
        : t("The task had already finished", "任务此前已结束")}
    </div>
  );
}

/**
 * 渲染 action=cleanup 的清理结果。
 *
 * @param props 结果对象与双语文本选择方法
 * @returns 清理结论行
 */
function CleanupOutcome({ result, t }: { result: JsonRecord; t: Translate }) {
  const removed = Array.isArray(result.removed) ? result.removed.length : 0;
  const remaining = typeof result.remaining === "number" ? result.remaining : 0;
  return (
    <div className="bg-task-note">
      {t(`Removed ${removed} finished ${removed === 1 ? "task" : "tasks"}; ${remaining} remaining`, `已清理 ${removed} 个已结束任务，剩余 ${remaining} 个`)}
    </div>
  );
}

/**
 * 读取对象中的对象字段。
 *
 * @param record JSON 对象
 * @param key 字段名
 * @returns 对象字段或空值
 */
function recordField(record: JsonRecord | null, key: string): JsonRecord | null {
  const value = record?.[key];
  return isRecord(value) ? value : null;
}

/**
 * 判断未知值是否为普通对象。
 *
 * @param value 待判断值
 * @returns 是否可按 JSON 对象读取
 */
function isRecord(value: unknown): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
