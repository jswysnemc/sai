import { useEffect, useMemo, useRef, useState } from "react";
import { ChoicePills, SettingsPanel } from "../kit";
import type { UsageDailyActivity } from "../../../api/contracts/usage";
import type { Translate } from "./usage-labels";
import { formatTokens } from "./usage-format";
import {
  HEAT_COLORS,
  HEAT_CELL_GAP,
  HEAT_LABEL_WIDTH,
  TOOLTIP_WIDTH,
  WEEK_COUNT,
  buildHeatmapDays,
  buildMonthLabels,
  calculateHeatCellSize,
  formatDateLabel,
  type HeatmapDay,
  type HeatmapMode
} from "./usage-activity";
import "./usage-heatmap.css";

type Props = {
  days: UsageDailyActivity[];
  locale: "en-US" | "zh-CN";
  t: Translate;
};

const WEEKDAY_KEYS = [
  ["Mon", "一"],
  ["Wed", "三"],
  ["Fri", "五"]
] as const;

/**
 * 【用量】【热力图】52 周日历，支持日 / 周 / 累计，空日占位且不含未来。
 * @param props 连续日历、语言与双语函数
 * @returns 热力图面板
 */
export function UsageHeatmap({ days, locale, t }: Props) {
  const measureRef = useRef<HTMLDivElement | null>(null);
  const [mode, setMode] = useState<HeatmapMode>("daily");
  const [hoveredDate, setHoveredDate] = useState<string | null>(null);
  const [focusedDate, setFocusedDate] = useState<string | null>(null);
  const [heatCellSize, setHeatCellSize] = useState(10);

  useEffect(() => {
    const element = measureRef.current;
    if (!element) return;
    const update = () => {
      const next = calculateHeatCellSize(element.clientWidth);
      setHeatCellSize((current) => (current === next ? current : next));
    };
    update();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", update);
      return () => window.removeEventListener("resize", update);
    }
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [days.length]);

  const cells = useMemo(() => buildHeatmapDays(days, mode), [days, mode]);
  const monthLabels = useMemo(() => buildMonthLabels(cells, locale), [cells, locale]);
  const activeDate = hoveredDate ?? focusedDate;
  const tooltipDay = cells.find((day) => day.date === activeDate) ?? null;
  const tooltipIndex = tooltipDay ? cells.findIndex((day) => day.date === tooltipDay.date) : -1;
  const heatGridWidth = WEEK_COUNT * heatCellSize + (WEEK_COUNT - 1) * HEAT_CELL_GAP;
  const heatGridHeight = 7 * heatCellSize + 6 * HEAT_CELL_GAP;
  const heatmapWidth = HEAT_LABEL_WIDTH + heatGridWidth;
  const tooltipStyle = tooltipIndex >= 0
    ? {
        left: Math.max(
          HEAT_LABEL_WIDTH,
          Math.min(
            heatmapWidth - TOOLTIP_WIDTH,
            HEAT_LABEL_WIDTH + Math.floor(tooltipIndex / 7) * (heatCellSize + HEAT_CELL_GAP) - 52
          )
        ),
        top: Math.max(28, 30 + (tooltipIndex % 7) * (heatCellSize + HEAT_CELL_GAP) - 50)
      }
    : undefined;

  return (
    <SettingsPanel
      title={t("Token activity", "Token 活动")}
      actions={
        <ChoicePills
          value={mode}
          onChange={(next) => setMode(next)}
          ariaLabel={t("Heatmap mode", "热力图模式")}
          options={[
            { value: "daily", label: t("Daily", "按日"), title: t("Each cell is one day.", "每格一天。") },
            { value: "weekly", label: t("Weekly", "按周"), title: t("Each column is one week.", "每列一周。") },
            { value: "cumulative", label: t("Cumulative", "累计"), title: t("Running token total by week.", "按周累计 Token。") }
          ]}
        />
      }
    >
      <div ref={measureRef} className="usage-heatmap">
        <div className="usage-heatmap-frame" style={{ width: heatmapWidth, maxWidth: "100%" }}>
          <div data-testid="activity-month-labels" className="usage-heatmap-months" style={{ marginLeft: HEAT_LABEL_WIDTH }}>
            {monthLabels.map((month) => (
              <span key={`${month.week}-${month.label}`} style={{ left: month.week * (heatCellSize + HEAT_CELL_GAP) }}>
                {month.label}
              </span>
            ))}
          </div>
          <div className="usage-heatmap-body" style={{ gap: HEAT_CELL_GAP }}>
            <div className="usage-heatmap-weekdays" style={{ width: HEAT_LABEL_WIDTH, height: heatGridHeight, rowGap: HEAT_CELL_GAP }}>
              <div className="row-start-2">{t(WEEKDAY_KEYS[0][0], WEEKDAY_KEYS[0][1])}</div>
              <div className="row-start-4">{t(WEEKDAY_KEYS[1][0], WEEKDAY_KEYS[1][1])}</div>
              <div className="row-start-6">{t(WEEKDAY_KEYS[2][0], WEEKDAY_KEYS[2][1])}</div>
            </div>
            <div
              role="grid"
              aria-label={t("Token activity", "Token 活动")}
              className="usage-heatmap-grid"
              style={{
                gridTemplateRows: `repeat(7, ${heatCellSize}px)`,
                gridAutoColumns: `${heatCellSize}px`,
                columnGap: HEAT_CELL_GAP,
                rowGap: HEAT_CELL_GAP
              }}
              onMouseLeave={() => setHoveredDate(null)}
            >
              {cells.map((day, index) => (
                <HeatCell
                  key={day.date}
                  day={day}
                  size={heatCellSize}
                  selected={activeDate === day.date}
                  tabStop={activeDate ? activeDate === day.date : index === cells.length - 1}
                  locale={locale}
                  t={t}
                  onFocus={() => setFocusedDate(day.date)}
                  onBlur={() => setFocusedDate(null)}
                  onEnter={() => setHoveredDate(day.date)}
                />
              ))}
            </div>
          </div>
          {tooltipDay && (
            <div
              id={`activity-day-tooltip-${tooltipDay.date}`}
              role="tooltip"
              className="usage-heatmap-tooltip"
              style={tooltipStyle}
            >
              <div>{cellTitle(tooltipDay, locale, t)}</div>
              <div>{cellDetail(tooltipDay, t)}</div>
            </div>
          )}
        </div>
        <div className="usage-heatmap-legend">
          <span>{t("Less", "少")}</span>
          {HEAT_COLORS.map((color) => (
            <i key={color} aria-hidden="true" style={{ width: heatCellSize, height: heatCellSize, background: color }} />
          ))}
          <span>{t("More", "多")}</span>
        </div>
      </div>
    </SettingsPanel>
  );
}

type HeatCellProps = {
  day: HeatmapDay;
  size: number;
  selected: boolean;
  /** 仅当前格子进入 Tab 序列，避免 52 周格子逐个抢焦点 */
  tabStop: boolean;
  locale: "en-US" | "zh-CN";
  t: Translate;
  onFocus: () => void;
  onBlur: () => void;
  onEnter: () => void;
};

/**
 * 渲染单个热力格子。
 * @param props 格子数据、尺寸、Tab 停留点与交互
 * @returns 可聚焦的格子按钮
 */
function HeatCell({ day, size, selected, tabStop, locale, t, onFocus, onBlur, onEnter }: HeatCellProps) {
  const title = cellTitle(day, locale, t);
  const detail = cellDetail(day, t);
  return (
    <button
      type="button"
      role="gridcell"
      tabIndex={tabStop ? 0 : -1}
      aria-label={`${title}: ${detail}`}
      aria-describedby={selected ? `activity-day-tooltip-${day.date}` : undefined}
      data-date={day.date}
      className={selected ? "usage-heat-cell is-active" : "usage-heat-cell"}
      style={{ width: size, height: size, backgroundColor: HEAT_COLORS[day.level] }}
      onFocus={onFocus}
      onBlur={onBlur}
      onMouseEnter={onEnter}
    />
  );
}

/**
 * 格子标题：日模式给当天，周 / 累计给出区间。
 * @param day 格子
 * @param locale 界面语言
 * @param t 双语函数
 * @returns 标题
 */
function cellTitle(day: HeatmapDay, locale: "en-US" | "zh-CN", t: Translate): string {
  if (day.mode === "weekly") {
    return t(
      `${formatDateLabel(day.rangeStart ?? day.date, locale)} – ${formatDateLabel(day.rangeEnd ?? day.date, locale)}`,
      `${formatDateLabel(day.rangeStart ?? day.date, locale)} – ${formatDateLabel(day.rangeEnd ?? day.date, locale)}`
    );
  }
  if (day.mode === "cumulative") {
    return t(
      `Through ${formatDateLabel(day.rangeEnd ?? day.date, locale)}`,
      `截至 ${formatDateLabel(day.rangeEnd ?? day.date, locale)}`
    );
  }
  return formatDateLabel(day.date, locale);
}

/**
 * 格子详情：累计只报 Token，其余给出会话数。
 * @param day 格子
 * @param t 双语函数
 * @returns 详情
 */
function cellDetail(day: HeatmapDay, t: Translate): string {
  if (day.mode === "cumulative") {
    return t(`${formatTokens(day.tokens)} tokens`, `${formatTokens(day.tokens)} Token`);
  }
  return t(
    `${day.sessions} sessions · ${formatTokens(day.tokens)} tokens`,
    `${day.sessions} 个会话 · ${formatTokens(day.tokens)} Token`
  );
}
