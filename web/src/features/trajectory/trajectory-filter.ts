import type { TrajectoryRecord, TrajectoryRecordKind } from "./trajectory-record";
import { recordOverlaps } from "./trajectory-record";
import type { TimeDomain } from "./trajectory-scale";

/** 记录表的筛选条件。 */
export type TrajectoryFilter = {
  /** 搜索词；空串表示不筛选 */
  query: string;
  /** 被隐藏的记录种类 */
  hiddenKinds: ReadonlySet<TrajectoryRecordKind>;
  /** 概览拖选出的时间区间；null 表示不筛选 */
  range: TimeDomain | null;
  /** 被折叠的轮次标识 */
  collapsedTurns: ReadonlySet<string>;
};

/**
 * 按搜索词、种类、时间区间与折叠状态筛选记录。
 *
 * 折叠轮次保留首条记录：整轮消失会让上下文断裂，
 * 留一条带计数的入口才能看出这里折了什么。
 *
 * @param records 全部记录
 * @param filter 筛选条件
 * @returns 通过筛选的记录
 */
export function filterRecords(
  records: readonly TrajectoryRecord[],
  filter: TrajectoryFilter
): TrajectoryRecord[] {
  const needle = filter.query.trim().toLowerCase();
  return records.filter((record) => {
    if (filter.hiddenKinds.has(record.kind)) return false;
    if (record.turnId && filter.collapsedTurns.has(record.turnId) && !record.turnStart) return false;
    if (filter.range && !recordOverlaps(record, filter.range.from, filter.range.to)) return false;
    if (needle && !matchesQuery(record, needle)) return false;
    return true;
  });
}

/**
 * 判断记录是否命中搜索词。
 *
 * 详情里的入参与输出一并参与匹配：只搜摘要的话，
 * 找一个只出现在工具输出里的路径就永远搜不到。
 *
 * @param record 轨迹记录
 * @param needle 已转小写的搜索词
 * @returns 是否命中
 */
function matchesQuery(record: TrajectoryRecord, needle: string): boolean {
  if (record.summary.toLowerCase().includes(needle)) return true;
  if (record.label?.toLowerCase().includes(needle)) return true;
  const detail = record.detail;
  return Boolean(
    detail.input?.toLowerCase().includes(needle)
    || detail.output?.toLowerCase().includes(needle)
    || detail.reasoning?.toLowerCase().includes(needle)
    || detail.error?.toLowerCase().includes(needle)
  );
}

/**
 * 统计每个轮次被折叠掉的记录数量。
 *
 * @param records 全部记录
 * @returns 轮次标识到记录总数的映射
 */
export function countByTurn(records: readonly TrajectoryRecord[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const record of records) {
    if (!record.turnId) continue;
    counts.set(record.turnId, (counts.get(record.turnId) ?? 0) + 1);
  }
  return counts;
}
