/**
 * 摘要文本的一帧内容。
 */
export type SummaryFrame = {
  key: string;
  primaryText: string;
  secondaryText: string;
};

/** 一帧文本至少停留的时长 */
export const FRAME_HOLD_MS = 800;
/** 超过此时长仍在排队时只保留最后一帧，避免落后太多 */
export const FRAME_CATCH_UP_MS = 250;
/** 最多缓存的待播帧数 */
export const MAX_QUEUED_FRAMES = 2;

/**
 * 计算落后时应当保留的待播帧。
 *
 * 工具状态可能在极短时间内连跳数次（准备→运行→完成）。若逐帧播完，
 * 界面显示的状态会明显滞后于实际状态；因此落后超过阈值时直接跳到最后一帧。
 *
 * @param queued 当前待播帧
 * @param elapsedMs 距上一帧落地已过去的时长
 * @returns 应当保留的待播帧
 */
export function framesToPlay(queued: readonly SummaryFrame[], elapsedMs: number): SummaryFrame[] {
  if (elapsedMs > FRAME_CATCH_UP_MS && queued.length > 1) return queued.slice(-1);
  return [...queued];
}

/**
 * 将新帧并入待播队列。
 *
 * 同 key 视为同一帧，重复到达时忽略：流式更新会反复推送同一状态，
 * 不去重会让队列被同一内容占满，真正的新状态反而排不进来。
 *
 * @param queued 当前待播帧
 * @param frame 新到达的帧
 * @returns 并入后的待播帧
 */
export function enqueueFrame(
  queued: readonly SummaryFrame[],
  frame: SummaryFrame
): SummaryFrame[] {
  if (queued.some((item) => item.key === frame.key)) return [...queued];
  if (queued.length === 0) return [frame];
  // 只保留最早的一帧与最新一帧：中间态跳过不影响用户理解
  return [queued[0], frame].slice(0, MAX_QUEUED_FRAMES);
}
