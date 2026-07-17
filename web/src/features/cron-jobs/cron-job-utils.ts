import type { CronJob } from "../../api/contracts";

/**
 * 将 Unix 秒格式化为本地日期时间。
 *
 * @param timestamp Unix 秒
 * @returns 本地日期时间文本
 */
export function formatCronDate(timestamp: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  }).format(new Date(timestamp * 1_000));
}

/**
 * 将任务间隔格式化为便于阅读的文本。
 *
 * @param intervalSeconds 间隔秒数
 * @returns 间隔描述
 */
export function formatCronInterval(intervalSeconds?: number | null): string {
  if (!intervalSeconds) return "单次执行";
  if (intervalSeconds % 86_400 === 0) return `每 ${intervalSeconds / 86_400} 天`;
  if (intervalSeconds % 3_600 === 0) return `每 ${intervalSeconds / 3_600} 小时`;
  if (intervalSeconds % 60 === 0) return `每 ${intervalSeconds / 60} 分钟`;
  return `每 ${intervalSeconds} 秒`;
}

/**
 * 根据任务状态返回界面状态文本。
 *
 * @param job 定时任务
 * @returns 状态文本
 */
export function getCronJobStatus(job: CronJob): string {
  if (!job.enabled && job.failure_count >= 3) return "失败后停用";
  return job.enabled ? "已启用" : "已停用";
}
