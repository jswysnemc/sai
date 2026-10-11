import type { UsageDailyActivity } from "../../../api/contracts/usage";

export type HeatmapMode = "daily" | "weekly" | "cumulative";

export type HeatmapDay = {
  date: string;
  requests: number;
  sessions: number;
  tokens: number;
  level: number;
  mode: HeatmapMode;
  rangeStart?: string;
  rangeEnd?: string;
};

export const WEEK_COUNT = 52;
export const CHART_DAY_COUNT = 14;
export const MONTH_LABEL_MIN_WEEKS = 3;
export const HEAT_CELL_GAP = 3;
export const HEAT_LABEL_WIDTH = 38;
export const HEAT_CELL_MIN = 6;
export const HEAT_CELL_MAX = 22;
export const TOOLTIP_WIDTH = 172;

export const HEAT_COLORS = [
  "var(--usage-heat-0)",
  "var(--usage-heat-1)",
  "var(--usage-heat-2)",
  "var(--usage-heat-3)",
  "var(--usage-heat-4)"
];

/**
 * 新鲜 Token：输入总量扣掉缓存读取后再加上输出。
 * 输入字段已含缓存命中，不能把 input + output 当成新鲜量。
 *
 * @param input 上报输入量
 * @param output 输出量
 * @param cached 缓存读取量
 * @returns 非负新鲜量
 */
export function freshTokens(input: number, output: number, cached: number): number {
  return Math.max(0, input - cached) + output;
}

/**
 * 把日历日解析为本地零点。
 *
 * @param dateKey YYYY-MM-DD
 * @returns 本地日期
 */
export function parseLocalDate(dateKey: string): Date {
  return new Date(`${dateKey}T00:00:00`);
}

/**
 * 格式化完整日期。
 *
 * @param dateKey YYYY-MM-DD
 * @param locale 界面语言
 * @returns 本地化日期
 */
export function formatDateLabel(dateKey: string, locale: "en-US" | "zh-CN"): string {
  return parseLocalDate(dateKey).toLocaleDateString(locale, {
    month: "short",
    day: "numeric",
    year: "numeric"
  });
}

/**
 * 格式化月日。
 *
 * @param dateKey YYYY-MM-DD
 * @param locale 界面语言
 * @returns 月日文本
 */
export function formatShortDate(dateKey: string, locale: "en-US" | "zh-CN"): string {
  return parseLocalDate(dateKey).toLocaleDateString(locale, {
    month: "numeric",
    day: "numeric"
  });
}

/**
 * 按可用宽度计算热力格子边长。
 *
 * @param width 容器像素宽
 * @returns 夹紧后的边长
 */
export function calculateHeatCellSize(width: number): number {
  const available = width - HEAT_LABEL_WIDTH - (WEEK_COUNT - 1) * HEAT_CELL_GAP;
  return Math.max(HEAT_CELL_MIN, Math.min(HEAT_CELL_MAX, Math.floor(available / WEEK_COUNT)));
}

function dayScore(day: UsageDailyActivity): number {
  return day.sessions * 3 + Math.ceil(day.tokens / 50_000);
}

/**
 * 把单日分数映射到 0–4 热力档。
 *
 * @param day 当日记录
 * @param maxScore 窗口内最高分
 * @returns 热力档
 */
export function getHeatLevel(day: UsageDailyActivity, maxScore: number): number {
  if (day.requests === 0 && day.tokens === 0) return 0;
  if (maxScore <= 0) return 1;
  const ratio = dayScore(day) / maxScore;
  if (ratio >= 0.78) return 4;
  if (ratio >= 0.5) return 3;
  if (ratio >= 0.24) return 2;
  return 1;
}

function getBarHeight(value: number, maxValue: number): number {
  if (value <= 0 || maxValue <= 0) return 0;
  return Math.max(1, Math.min(7, Math.ceil((value / maxValue) * 7)));
}

function getBarLevel(value: number, maxValue: number): number {
  if (value <= 0) return 0;
  if (maxValue <= 0) return 1;
  const ratio = value / maxValue;
  if (ratio >= 0.78) return 4;
  if (ratio >= 0.5) return 3;
  if (ratio >= 0.24) return 2;
  return 1;
}

/**
 * 把后端连续日历转成热力图格子。
 *
 * 日模式按日上色；周 / 累计模式把一周压成柱，从格子底部往上填。
 *
 * @param days 已按日期升序、含空日的日历
 * @param mode 日 / 周 / 累计
 * @returns 与日历等长的格子
 */
export function buildHeatmapDays(days: UsageDailyActivity[], mode: HeatmapMode): HeatmapDay[] {
  if (days.length === 0) return [];
  // 1. 先按日上色，作为周 / 累计模式的底稿
  const maxScore = Math.max(0, ...days.map(dayScore));
  const daily: HeatmapDay[] = days.map((day) => ({
    date: day.date,
    requests: day.requests,
    sessions: day.sessions,
    tokens: day.tokens,
    level: getHeatLevel(day, maxScore),
    mode: "daily"
  }));
  if (mode === "daily") return daily;

  // 2. 每 7 格合成一列，累计从窗口起点滚加
  const weekCount = Math.ceil(days.length / 7);
  const weeks = Array.from({ length: weekCount }, (_, index) => {
    const slice = days.slice(index * 7, index * 7 + 7);
    return {
      rangeStart: slice[0]?.date ?? "",
      rangeEnd: slice[slice.length - 1]?.date ?? "",
      requests: slice.reduce((sum, day) => sum + day.requests, 0),
      sessions: slice.reduce((sum, day) => sum + day.sessions, 0),
      tokens: slice.reduce((sum, day) => sum + day.tokens, 0),
      cumulativeTokens: 0
    };
  });
  let running = 0;
  for (const week of weeks) {
    running += week.tokens;
    week.cumulativeTokens = running;
  }
  // 3. 从格子底部往上填柱，空行保持 0 档
  const maxValue = Math.max(0, ...weeks.map((week) => (mode === "weekly" ? week.tokens : week.cumulativeTokens)));
  return days.map((day, index) => {
    const week = weeks[Math.floor(index / 7)];
    const row = index % 7;
    const tokens = mode === "weekly" ? week?.tokens ?? 0 : week?.cumulativeTokens ?? 0;
    const height = getBarHeight(tokens, maxValue);
    const isFilled = height > 0 && row >= 7 - height;
    return {
      date: day.date,
      requests: week?.requests ?? 0,
      sessions: week?.sessions ?? 0,
      tokens,
      level: isFilled ? getBarLevel(tokens, maxValue) : 0,
      mode,
      rangeStart: week?.rangeStart,
      rangeEnd: week?.rangeEnd
    };
  });
}

/**
 * 生成不碰撞、不越出右缘的月份标签。
 *
 * @param days 热力格子
 * @param locale 界面语言
 * @returns 周列索引与月份文案
 */
export function buildMonthLabels(days: HeatmapDay[], locale: "en-US" | "zh-CN"): Array<{ week: number; label: string }> {
  if (days.length === 0) return [];
  const labels: Array<{ week: number; label: string }> = [];
  const firstDate = parseLocalDate(days[0].date);
  const lastDate = parseLocalDate(days[days.length - 1].date);
  let previousMonth = -1;
  for (let week = 0; week < WEEK_COUNT - (MONTH_LABEL_MIN_WEEKS - 1); week += 1) {
    const weekDate = new Date(firstDate);
    weekDate.setDate(firstDate.getDate() + week * 7);
    if (weekDate > lastDate) break;
    if (weekDate.getMonth() !== previousMonth) {
      const previous = labels[labels.length - 1];
      if (previous && week - previous.week < MONTH_LABEL_MIN_WEEKS) labels.pop();
      labels.push({
        week,
        label: weekDate.toLocaleDateString(locale, { month: "short" })
      });
      previousMonth = weekDate.getMonth();
    }
  }
  return labels;
}

/**
 * 截取窗口末尾若干天，供近况柱图使用。
 *
 * @param days 连续日历
 * @param count 天数
 * @returns 末尾若干天
 */
export function buildRecentDays(days: UsageDailyActivity[], count = CHART_DAY_COUNT): UsageDailyActivity[] {
  if (days.length <= count) return days;
  return days.slice(-count);
}
