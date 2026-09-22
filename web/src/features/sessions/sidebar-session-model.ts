import type { Session, WorkspaceSessions } from "../../api/contracts";

/** 跨工作区摊平后的一条会话。 */
export type SidebarSessionRef = {
  workspaceId: string;
  workspaceName: string;
  workspacePath: string;
  workspaceActive: boolean;
  session: Session;
};

export type TaskSort = "updated" | "created";

export type TimelineBucket = {
  id: string;
  labelEn: string;
  labelZh: string;
  items: SidebarSessionRef[];
};

/**
 * 把各工作区的会话摊成一份列表。
 *
 * @param workspaces 工作区及其会话
 * @returns 带工作区信息的会话
 */
export function flattenSessions(workspaces: WorkspaceSessions[]): SidebarSessionRef[] {
  return workspaces.flatMap((workspace) => workspace.sessions.map((session) => ({
    workspaceId: workspace.workspace_id,
    workspaceName: workspace.workspace_name,
    workspacePath: workspace.workspace_path,
    workspaceActive: workspace.active,
    session
  })));
}

/**
 * 按更新时间或创建时间排序，新的在前。
 *
 * @param items 会话
 * @param sort 排序字段
 * @returns 新数组
 */
export function sortSessions(items: SidebarSessionRef[], sort: TaskSort): SidebarSessionRef[] {
  const key = sort === "created" ? "created_at" : "updated_at";
  return [...items].sort((left, right) => right.session[key].localeCompare(left.session[key]));
}

/**
 * 按日期把会话分到时间线桶。
 *
 * @param items 已排序的会话
 * @param now 当前时间毫秒
 * @returns 非空的日期桶，顺序固定
 */
export function bucketSessions(items: SidebarSessionRef[], now: number): TimelineBucket[] {
  const buckets = new Map<string, SidebarSessionRef[]>();
  for (const item of items) {
    const id = timelineBucketId(item.session.updated_at, now);
    const list = buckets.get(id) ?? [];
    list.push(item);
    buckets.set(id, list);
  }
  return TIMELINE_BUCKETS.flatMap((bucket) => {
    const grouped = buckets.get(bucket.id);
    return grouped?.length ? [{ ...bucket, items: grouped }] : [];
  });
}

const TIMELINE_BUCKETS: Array<Omit<TimelineBucket, "items">> = [
  { id: "today", labelEn: "Today", labelZh: "今天" },
  { id: "yesterday", labelEn: "Yesterday", labelZh: "昨天" },
  { id: "this-week", labelEn: "This week", labelZh: "本周" },
  { id: "last-week", labelEn: "Last week", labelZh: "上周" },
  { id: "this-month", labelEn: "This month", labelZh: "本月" },
  { id: "last-month", labelEn: "Last month", labelZh: "上月" },
  { id: "earlier", labelEn: "Earlier", labelZh: "更早" }
];

/**
 * 判断一条时间落在哪个时间线桶。
 *
 * @param value ISO 时间
 * @param now 当前时间毫秒
 * @returns 桶标识
 */
export function timelineBucketId(value: string, now: number): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "earlier";
  const current = new Date(now);
  const days = calendarDaysBetween(date, current);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  const weekStart = startOfWeek(current);
  if (date >= weekStart) return "this-week";
  const lastWeek = new Date(weekStart);
  lastWeek.setDate(lastWeek.getDate() - 7);
  if (date >= lastWeek) return "last-week";
  if (date.getFullYear() === current.getFullYear() && date.getMonth() === current.getMonth()) return "this-month";
  const previous = new Date(current.getFullYear(), current.getMonth() - 1, 1);
  if (date.getFullYear() === previous.getFullYear() && date.getMonth() === previous.getMonth()) return "last-month";
  return "earlier";
}

/**
 * 计算两个本地日期相差的整天数。
 *
 * @param earlier 较早的时间
 * @param later 较晚的时间
 * @returns 天数，同一天为 0
 */
function calendarDaysBetween(earlier: Date, later: Date): number {
  const start = new Date(earlier.getFullYear(), earlier.getMonth(), earlier.getDate()).getTime();
  const end = new Date(later.getFullYear(), later.getMonth(), later.getDate()).getTime();
  return Math.round((end - start) / 86_400_000);
}

/**
 * 返回本周一的零点。
 *
 * @param date 参考时间
 * @returns 本周起始
 */
function startOfWeek(date: Date): Date {
  const start = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const weekday = start.getDay() || 7;
  start.setDate(start.getDate() - weekday + 1);
  return start;
}
