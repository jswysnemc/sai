import type { BackgroundTask } from "../../api/contracts";

/** 判断后台任务是否仍在运行。 */
export function isBackgroundTaskRunning(task: BackgroundTask): boolean {
  return task.status === "running";
}

/** 将后台任务状态转换为界面文字。 */
export function backgroundTaskStatusLabel(status: string): string {
  return ({ running: "运行中", exited: "已结束", stopped: "已停止", timed_out: "已超时" } as Record<string, string>)[status] ?? status;
}

/** 格式化后台任务从启动到当前或结束时刻的运行时长。 */
export function formatBackgroundTaskDuration(task: BackgroundTask, nowSeconds = Math.floor(Date.now() / 1000)): string {
  const end = isBackgroundTaskRunning(task) ? nowSeconds : task.updated_at;
  const total = Math.max(0, end - task.started_at);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) return `${hours}小时 ${minutes}分`;
  if (minutes > 0) return `${minutes}分 ${seconds}秒`;
  return `${seconds}秒`;
}

/** 合并标准输出和错误输出，保留输出流标签。 */
export function combineBackgroundTaskOutput(stdout?: string | null, stderr?: string | null): string {
  const sections = [];
  if (stdout) sections.push(`stdout\n${stdout}`);
  if (stderr) sections.push(`stderr\n${stderr}`);
  return sections.join("\n\n");
}
