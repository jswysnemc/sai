import type { CronJob } from "../../api/contracts";
import { text, type Locale } from "../i18n/locale";

/**
 * 将 Unix 秒格式化为本地日期时间。
 *
 * @param timestamp Unix 秒
 * @param locale 界面语言
 * @returns 本地日期时间文本
 */
export function formatCronDate(timestamp: number, locale: Locale = "zh-CN"): string {
  return new Intl.DateTimeFormat(locale, {
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
 * @param locale 界面语言
 * @returns 间隔描述
 */
export function formatCronInterval(intervalSeconds?: number | null, locale: Locale = "zh-CN"): string {
  if (!intervalSeconds) return text(locale, "Runs once", "单次执行");
  if (intervalSeconds % 86_400 === 0) return text(locale, `Every ${intervalSeconds / 86_400} days`, `每 ${intervalSeconds / 86_400} 天`);
  if (intervalSeconds % 3_600 === 0) return text(locale, `Every ${intervalSeconds / 3_600} hours`, `每 ${intervalSeconds / 3_600} 小时`);
  if (intervalSeconds % 60 === 0) return text(locale, `Every ${intervalSeconds / 60} minutes`, `每 ${intervalSeconds / 60} 分钟`);
  return text(locale, `Every ${intervalSeconds} seconds`, `每 ${intervalSeconds} 秒`);
}

/**
 * 根据任务状态返回界面状态文本。
 *
 * @param job 定时任务
 * @param locale 界面语言
 * @returns 状态文本
 */
export function getCronJobStatus(job: CronJob, locale: Locale = "zh-CN"): string {
  if (!job.enabled && job.failure_count >= 3) return text(locale, "Disabled after failures", "失败后停用");
  return job.enabled ? text(locale, "Enabled", "已启用") : text(locale, "Disabled", "已停用");
}
