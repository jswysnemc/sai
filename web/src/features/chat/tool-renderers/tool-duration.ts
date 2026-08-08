/*
 * 工具调用耗时
 *
 * 折叠行右侧只有很窄的一段空间，耗时必须在一瞥之间读完，
 * 因此按量级切换单位而不是统一到某一档：毫秒级用整数毫秒，
 * 秒级保留一位小数，分钟以上退回整秒并补分钟前缀。
 */

/** 低于这个毫秒数的调用不展示耗时：快到没有信息量，只会让折叠行变吵 */
const MIN_VISIBLE_MS = 200;

/**
 * 将毫秒时长格式化为折叠行可直接展示的短文本。
 *
 * @param ms 毫秒时长
 * @returns 形如 320ms / 1.2s / 2m10s 的短文本；时长无效时返回空串
 */
export function formatToolDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "";
  if (ms < 1_000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1_000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds - minutes * 60);
  // 取整后满 60 秒时进位到分钟，避免出现 "2m60s"
  if (rest === 60) return `${minutes + 1}m`;
  return rest === 0 ? `${minutes}m` : `${minutes}m${rest}s`;
}

/**
 * 计算一次工具调用应展示的耗时文本。
 *
 * 已结束的调用取起止差值；仍在执行的调用用调用方传入的当前时间做差，
 * 因此计时的推进由调用方的刷新节奏决定，本函数保持纯函数。
 *
 * @param startedAtMs 开始毫秒时间戳
 * @param endedAtMs 结束毫秒时间戳；仍在执行时传 undefined
 * @param nowMs 当前毫秒时间戳，仅在未结束时使用
 * @returns 耗时短文本；不足展示阈值或缺少起点时返回空串
 */
export function toolDurationLabel(
  startedAtMs: number | undefined,
  endedAtMs: number | undefined,
  nowMs: number
): string {
  if (startedAtMs === undefined) return "";
  const end = endedAtMs ?? nowMs;
  const elapsed = end - startedAtMs;
  if (elapsed < MIN_VISIBLE_MS) return "";
  return formatToolDuration(elapsed);
}
