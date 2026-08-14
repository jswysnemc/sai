import type { TrajectoryRecord, TrajectoryRecordKind } from "./trajectory-record";
import { recordEndedAt } from "./trajectory-record";

/** 概览的横轴口径。 */
export type TrajectoryScaleMode = "duration" | "sequence";

/** 一段时间域，单位为毫秒时间戳。 */
export type TimeDomain = { from: number; to: number };

/** 概览轨道上的一段，位置与宽度为 0-100 的百分比。 */
export type TrackSegment = {
  id: string;
  index: number;
  kind: TrajectoryRecordKind;
  left: number;
  width: number;
  failed: boolean;
  running: boolean;
  /** 是否只有时刻没有跨度；界面用更细的刻线表示 */
  instant: boolean;
};

/** 无跨度记录在真实时间口径下的最小可见宽度百分比。 */
const INSTANT_WIDTH = 0.4;

/**
 * 求记录集合的时间域。
 *
 * @param records 轨迹记录
 * @returns 时间域；没有任何可用时刻时返回 null
 */
export function trajectoryDomain(records: readonly TrajectoryRecord[]): TimeDomain | null {
  let from = Number.POSITIVE_INFINITY;
  let to = Number.NEGATIVE_INFINITY;
  for (const record of records) {
    if (record.startedAt == null) continue;
    from = Math.min(from, record.startedAt);
    to = Math.max(to, recordEndedAt(record) ?? record.startedAt);
  }
  if (!Number.isFinite(from) || !Number.isFinite(to)) return null;
  // 全部记录同一时刻时给出一个最小域，避免除零
  return to > from ? { from, to } : { from, to: from + 1 };
}

/**
 * 把记录投影到概览轨道。
 *
 * @param records 轨迹记录
 * @param mode 横轴口径
 * @param domain 当前视窗时间域；等宽口径下忽略
 * @returns 轨道段落；真实时间口径下缺少时刻的记录会被跳过
 */
export function trackSegments(
  records: readonly TrajectoryRecord[],
  mode: TrajectoryScaleMode,
  domain: TimeDomain | null
): TrackSegment[] {
  if (mode === "sequence") return sequenceSegments(records);
  if (!domain) return [];
  const span = domain.to - domain.from;
  const segments: TrackSegment[] = [];
  for (const record of records) {
    if (record.startedAt == null) continue;
    const end = recordEndedAt(record) ?? record.startedAt;
    if (end < domain.from || record.startedAt > domain.to) continue;
    const left = ((record.startedAt - domain.from) / span) * 100;
    const rawWidth = ((end - record.startedAt) / span) * 100;
    const instant = rawWidth < INSTANT_WIDTH;
    segments.push({
      id: record.id,
      index: record.index,
      kind: record.kind,
      left: clampPercent(left),
      width: Math.min(100 - clampPercent(left), instant ? INSTANT_WIDTH : rawWidth),
      failed: record.failed,
      running: record.running,
      instant
    });
  }
  return segments;
}

/**
 * 按记录顺序等宽投影。
 *
 * @param records 轨迹记录
 * @returns 等宽轨道段落
 */
function sequenceSegments(records: readonly TrajectoryRecord[]): TrackSegment[] {
  if (records.length === 0) return [];
  const width = 100 / records.length;
  return records.map((record, position) => ({
    id: record.id,
    index: record.index,
    kind: record.kind,
    left: position * width,
    width,
    failed: record.failed,
    running: record.running,
    instant: false
  }));
}

/**
 * 把轨道上的横向比例换算成时间域内的时刻。
 *
 * @param ratio 0-1 的横向比例
 * @param domain 当前视窗时间域
 * @returns 对应的毫秒时间戳
 */
export function ratioToTime(ratio: number, domain: TimeDomain): number {
  return domain.from + (domain.to - domain.from) * Math.min(1, Math.max(0, ratio));
}

/**
 * 以某一点为锚缩放时间域。
 *
 * 锚点保持在原位：以视窗中心缩放会让指针下的内容滑走，
 * 在密集轨迹里几乎无法对准目标区间。
 *
 * @param domain 当前时间域
 * @param bounds 允许的最大时间域
 * @param anchorRatio 锚点在当前视窗中的 0-1 比例
 * @param factor 缩放系数；小于 1 为放大
 * @returns 缩放后的时间域
 */
export function zoomDomain(
  domain: TimeDomain,
  bounds: TimeDomain,
  anchorRatio: number,
  factor: number
): TimeDomain {
  const span = domain.to - domain.from;
  const maxSpan = bounds.to - bounds.from;
  const minSpan = Math.max(1, maxSpan / 5000);
  const nextSpan = Math.min(maxSpan, Math.max(minSpan, span * factor));
  const anchor = domain.from + span * anchorRatio;
  let from = anchor - nextSpan * anchorRatio;
  let to = from + nextSpan;
  if (from < bounds.from) {
    from = bounds.from;
    to = from + nextSpan;
  }
  if (to > bounds.to) {
    to = bounds.to;
    from = to - nextSpan;
  }
  return { from, to };
}

/**
 * 平移时间域。
 *
 * @param domain 当前时间域
 * @param bounds 允许的最大时间域
 * @param deltaRatio 相对视窗宽度的位移比例
 * @returns 平移后的时间域
 */
export function panDomain(domain: TimeDomain, bounds: TimeDomain, deltaRatio: number): TimeDomain {
  const span = domain.to - domain.from;
  let from = domain.from + span * deltaRatio;
  from = Math.min(bounds.to - span, Math.max(bounds.from, from));
  return { from, to: from + span };
}

/**
 * 限制百分比落在 0-100 之间。
 *
 * @param value 原始百分比
 * @returns 截断后的百分比
 */
function clampPercent(value: number): number {
  return Math.min(100, Math.max(0, value));
}
