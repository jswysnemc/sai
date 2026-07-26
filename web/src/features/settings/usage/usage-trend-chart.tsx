import { useMemo } from "react";
import type { UsageTrendPoint } from "../../../api/contracts";
import { formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";

type UsageTrendChartProps = {
  points: UsageTrendPoint[];
  t: Translate;
};

const VIEW_WIDTH = 640;
const VIEW_HEIGHT = 180;
const PAD_LEFT = 44;
const PAD_RIGHT = 16;
const PAD_TOP = 10;
const PAD_BOTTOM = 22;

/**
 * 渲染 Token 趋势折线图。
 *
 * 同时画出上报总量与等效计费量两条线，缓存命中造成的差距直接可见。
 *
 * @param props 趋势点与双语取值函数
 * @returns 趋势图，无数据时返回空态提示
 */
export function UsageTrendChart({ points, t }: UsageTrendChartProps) {
  const geometry = useMemo(() => buildGeometry(points), [points]);

  if (points.length === 0) {
    return <div className="usage-empty">{t("No trend data", "暂无趋势数据")}</div>;
  }

  return (
    <div className="usage-trend">
      <svg viewBox={`0 0 ${VIEW_WIDTH} ${VIEW_HEIGHT}`} className="usage-trend-svg" role="img" aria-label={t("token trend", "Token 趋势")}>
        {[0, 0.5, 1].map((fraction) => {
          const y = PAD_TOP + geometry.plotHeight - fraction * geometry.plotHeight;
          return (
            <g key={fraction}>
              <line x1={PAD_LEFT} y1={y} x2={VIEW_WIDTH - PAD_RIGHT} y2={y} className="usage-grid" />
              <text x={PAD_LEFT - 6} y={y + 3.5} textAnchor="end" className="usage-axis">
                {formatTokens(geometry.maxTokens * fraction)}
              </text>
            </g>
          );
        })}
        {geometry.reportedPath && <path d={geometry.reportedPath} className="usage-trend-line reported" fill="none" />}
        {geometry.billablePath && <path d={geometry.billablePath} className="usage-trend-line billable" fill="none" />}
        {points.map((point, index) => (
          <circle
            key={point.date}
            cx={geometry.x(index)}
            cy={geometry.y(point.total_tokens)}
            r="2.5"
            className="usage-trend-dot"
          />
        ))}
        <text x={PAD_LEFT} y={VIEW_HEIGHT - 5} className="usage-axis">{points[0]?.label}</text>
        {points.length > 1 && (
          <text x={VIEW_WIDTH - PAD_RIGHT} y={VIEW_HEIGHT - 5} textAnchor="end" className="usage-axis">
            {points[points.length - 1]?.label}
          </text>
        )}
      </svg>
      <ul className="usage-trend-legend">
        <li><i className="reported" />{t("Reported", "上报量")}</li>
        <li><i className="billable" />{t("Billable", "计费量")}</li>
      </ul>
    </div>
  );
}

/**
 * 计算趋势图坐标映射与两条折线路径。
 *
 * @param points 趋势点
 * @returns 绘图所需的比例尺、路径与最大值
 */
function buildGeometry(points: UsageTrendPoint[]) {
  // 1. 纵轴上界取两种口径的最大值，保证两条线同尺度可比
  const maxTokens = Math.max(
    1,
    ...points.map((point) => point.total_tokens),
    ...points.map((point) => point.billable_input_tokens + point.output_tokens)
  );
  const plotHeight = VIEW_HEIGHT - PAD_TOP - PAD_BOTTOM;
  const step = points.length > 1 ? (VIEW_WIDTH - PAD_LEFT - PAD_RIGHT) / (points.length - 1) : 0;
  const singleX = PAD_LEFT + (VIEW_WIDTH - PAD_LEFT - PAD_RIGHT) / 2;
  const x = (index: number) => (points.length > 1 ? PAD_LEFT + step * index : singleX);
  const y = (value: number) => PAD_TOP + plotHeight - (value / maxTokens) * plotHeight;
  // 2. 分别生成上报量与计费量折线
  const toPath = (valueOf: (point: UsageTrendPoint) => number) =>
    points
      .map((point, index) => `${index === 0 ? "M" : "L"} ${x(index).toFixed(1)} ${y(valueOf(point)).toFixed(1)}`)
      .join(" ");
  return {
    maxTokens,
    plotHeight,
    x,
    y,
    reportedPath: toPath((point) => point.total_tokens),
    billablePath: toPath((point) => point.billable_input_tokens + point.output_tokens),
  };
}
