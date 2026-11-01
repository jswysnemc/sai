import type { BackgroundTask } from "../../api/contracts";
import { text, type Locale } from "../i18n/locale";

export type BackgroundTaskFilter = "all" | "running" | "finished";

/**
 * 按状态筛选后台任务，不改变排序。
 * @param tasks 已排序任务
 * @param filter 当前筛选条件
 * @returns 匹配条件的任务
 */
export function filterBackgroundTasks(tasks: readonly BackgroundTask[], filter: BackgroundTaskFilter): BackgroundTask[] {
  return tasks.filter((task) => filter === "all" || (filter === "running" ? isBackgroundTaskRunning(task) : !isBackgroundTaskRunning(task)));
}

/**
 * 判断后台任务是否仍在运行。
 *
 * @param task 后台任务
 * @returns 任务仍在运行时返回 true
 */
export function isBackgroundTaskRunning(task: BackgroundTask): boolean {
  return task.status === "running";
}

/**
 * 按运行状态和启动时间排列任务，不修改原始列表。
 * @param tasks 后台任务列表
 * @returns 运行中优先、同组按启动时间倒序排列的新列表
 */
export function sortBackgroundTasks(tasks: readonly BackgroundTask[]): BackgroundTask[] {
  return [...tasks].sort((left, right) =>
    Number(isBackgroundTaskRunning(right)) - Number(isBackgroundTaskRunning(left))
    || right.started_at - left.started_at
    || left.id.localeCompare(right.id)
  );
}

/**
 * 将后台任务状态转换为界面文字。
 *
 * @param status 后台任务状态
 * @param locale 当前界面语言
 * @returns 本地化状态文本
 */
export function backgroundTaskStatusLabel(status: string, locale: Locale = "zh-CN"): string {
  return ({
    running: text(locale, "Running", "运行中"),
    failed: text(locale, "Failed", "已失败"),
    exited: text(locale, "Exited", "已结束"),
    stopped: text(locale, "Stopped", "已停止"),
    timed_out: text(locale, "Timed out", "已超时")
  } as Record<string, string>)[status] ?? status;
}

/**
 * 返回任务状态的紧凑标记，未知状态不伪装为正常结束。
 * @param status 服务端真实状态
 * @returns 紧凑状态标记
 */
export function backgroundTaskStatusBadge(status: string): string {
  return ({ running: "RUN", exited: "EXIT", stopped: "STOP", timed_out: "TIME", failed: "FAIL" } as Record<string, string>)[status] ?? status;
}

/**
 * 格式化后台任务从启动到当前或结束时刻的运行时长。
 *
 * @param task 后台任务
 * @param nowSeconds 当前 Unix 秒
 * @param locale 当前界面语言
 * @returns 本地化运行时长
 */
export function formatBackgroundTaskDuration(task: BackgroundTask, nowSeconds = Math.floor(Date.now() / 1000), locale: Locale = "zh-CN"): string {
  const end = isBackgroundTaskRunning(task) ? nowSeconds : task.updated_at;
  const total = Math.max(0, end - task.started_at);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) return text(locale, `${hours}h ${minutes}m`, `${hours}小时 ${minutes}分`);
  if (minutes > 0) return text(locale, `${minutes}m ${seconds}s`, `${minutes}分 ${seconds}秒`);
  return text(locale, `${seconds}s`, `${seconds}秒`);
}

/**
 * 合并标准输出和错误输出，保留输出流标签。
 *
 * @param stdout 标准输出
 * @param stderr 标准错误
 * @returns 合并后的输出文本
 */
export function combineBackgroundTaskOutput(stdout?: string | null, stderr?: string | null): string {
  const sections = [];
  if (stdout) sections.push(`stdout\n${stdout}`);
  if (stderr) sections.push(`stderr\n${stderr}`);
  return sections.join("\n\n");
}
