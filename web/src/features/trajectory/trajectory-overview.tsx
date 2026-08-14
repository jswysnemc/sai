import { useCallback, useMemo, useRef, useState } from "react";
import { useI18n } from "../i18n/use-i18n";
import { formatClock, formatDuration } from "./trajectory-format";
import type { TrajectoryRecord } from "./trajectory-record";
import {
  panDomain,
  ratioToTime,
  trackSegments,
  zoomDomain,
  type TimeDomain,
  type TrajectoryScaleMode
} from "./trajectory-scale";
import "./trajectory-overview.css";

type TrajectoryOverviewProps = {
  records: readonly TrajectoryRecord[];
  mode: TrajectoryScaleMode;
  /** 全部记录的时间域；null 表示没有可用时刻 */
  bounds: TimeDomain | null;
  /** 当前拖选出的区间 */
  range: TimeDomain | null;
  onRangeChange: (range: TimeDomain | null) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
};

/** 判定为点击而非拖选的横向位移阈值。 */
const CLICK_SLOP = 0.004;

/**
 * 渲染记录的时间投影概览。
 *
 * 只画有真实时刻的记录：把缺时刻的记录补到轴上等于虚构耗时，
 * 概览一旦开始编造，它就不再能用来判断哪一步慢。
 *
 * @param props 记录集合、横轴口径与区间选择状态
 * @returns 概览轨道
 */
export function TrajectoryOverview({
  records,
  mode,
  bounds,
  range,
  onRangeChange,
  selectedId,
  onSelect
}: TrajectoryOverviewProps) {
  const { t, locale } = useI18n();
  const trackRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState<TimeDomain | null>(null);
  const [dragging, setDragging] = useState<{ from: number; to: number } | null>(null);
  const domain = mode === "duration" ? viewport ?? bounds : null;
  const segments = useMemo(
    () => trackSegments(records, mode, domain),
    [records, mode, domain]
  );

  /**
   * 把指针位置换算成轨道内的 0-1 比例。
   *
   * @param clientX 指针的视口横坐标
   * @returns 轨道内比例
   */
  const ratioAt = useCallback((clientX: number): number => {
    const box = trackRef.current?.getBoundingClientRect();
    if (!box || box.width === 0) return 0;
    return Math.min(1, Math.max(0, (clientX - box.left) / box.width));
  }, []);

  /**
   * 开始拖选区间。
   *
   * @param event 指针按下事件
   * @returns 无
   */
  const startDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || mode !== "duration" || !domain) return;
    const start = ratioAt(event.clientX);
    setDragging({ from: start, to: start });
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  /**
   * 更新拖选终点。
   *
   * @param event 指针移动事件
   * @returns 无
   */
  const moveDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!dragging) return;
    setDragging({ from: dragging.from, to: ratioAt(event.clientX) });
  };

  /**
   * 结束拖选并提交区间。
   *
   * 位移不足阈值时视为点击，清除已有区间而不是选出一个零宽窗口。
   *
   * @param event 指针抬起事件
   * @returns 无
   */
  const endDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!dragging || !domain) return;
    const { from, to } = dragging;
    setDragging(null);
    event.currentTarget.releasePointerCapture(event.pointerId);
    if (Math.abs(to - from) < CLICK_SLOP) {
      onRangeChange(null);
      return;
    }
    onRangeChange({
      from: ratioToTime(Math.min(from, to), domain),
      to: ratioToTime(Math.max(from, to), domain)
    });
  };

  /**
   * 滚轮缩放或平移时间域。
   *
   * @param event 滚轮事件
   * @returns 无
   */
  const handleWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    if (mode !== "duration" || !bounds || !domain) return;
    event.preventDefault();
    if (event.shiftKey) {
      setViewport(panDomain(domain, bounds, event.deltaY > 0 ? 0.12 : -0.12));
      return;
    }
    const next = zoomDomain(domain, bounds, ratioAt(event.clientX), event.deltaY > 0 ? 1.25 : 0.8);
    setViewport(next.to - next.from >= bounds.to - bounds.from ? null : next);
  };

  const selectionStyle = useMemo(() => {
    if (!domain) return null;
    const active = dragging
      ? { from: Math.min(dragging.from, dragging.to), to: Math.max(dragging.from, dragging.to) }
      : range
        ? {
            from: (range.from - domain.from) / (domain.to - domain.from),
            to: (range.to - domain.from) / (domain.to - domain.from)
          }
        : null;
    if (!active) return null;
    const left = Math.min(1, Math.max(0, active.from)) * 100;
    const right = Math.min(1, Math.max(0, active.to)) * 100;
    return { left: `${left}%`, width: `${Math.max(0, right - left)}%` };
  }, [dragging, range, domain]);

  const zoomed = viewport !== null;

  return (
    <div className="trajectory-overview">
      <div className="trajectory-overview-axis">
        <span>{domain ? formatClock(domain.from, locale) : ""}</span>
        {domain && <span className="trajectory-overview-span">{formatDuration(domain.to - domain.from)}</span>}
        {zoomed && (
          <button
            type="button"
            className="trajectory-overview-reset"
            onClick={() => { setViewport(null); onRangeChange(null); }}
          >
            {t("Reset zoom", "重置缩放")}
          </button>
        )}
        <span>{domain ? formatClock(domain.to, locale) : ""}</span>
      </div>
      <div
        className="trajectory-overview-track"
        ref={trackRef}
        role="group"
        aria-label={t("Trajectory time overview", "轨迹时间概览")}
        data-mode={mode}
        onPointerDown={startDrag}
        onPointerMove={moveDrag}
        onPointerUp={endDrag}
        onWheel={handleWheel}
        onContextMenu={(event) => { event.preventDefault(); onRangeChange(null); }}
      >
        {segments.length === 0 && (
          <span className="trajectory-overview-empty">{t("No timing recorded", "没有可用时刻")}</span>
        )}
        {segments.map((segment) => (
          <button
            type="button"
            key={segment.id}
            className="trajectory-overview-segment"
            style={{ left: `${segment.left}%`, width: `${segment.width}%` }}
            data-kind={segment.kind}
            data-instant={segment.instant || undefined}
            data-failed={segment.failed || undefined}
            data-running={segment.running || undefined}
            data-selected={segment.id === selectedId || undefined}
            aria-label={t(`Record ${segment.index}`, `第 ${segment.index} 条记录`)}
            onPointerDown={(event) => event.stopPropagation()}
            onClick={() => onSelect(segment.id)}
          />
        ))}
        {selectionStyle && (
          <span className="trajectory-overview-selection" style={selectionStyle} aria-hidden />
        )}
      </div>
    </div>
  );
}
