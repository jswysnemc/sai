import type { UsageGroupStats } from "../../../api/contracts";
import { formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";

type UsageModelDonutProps = {
  rows: UsageGroupStats[];
  t: Translate;
};

/** 图表序列色，取自主题派生的 token，随主题切换。 */
const SERIES_COLORS = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
  "var(--chart-6)",
];

const MAX_SLICES = 6;
const CENTER = 100;
const RADIUS_OUTER = 90;
const RADIUS_INNER = 54;

/**
 * 渲染模型 Token 占比环形图。
 *
 * @param props 模型分组统计与双语取值函数
 * @returns 环形图与图例，无数据时返回空态提示
 */
export function UsageModelDonut({ rows, t }: UsageModelDonutProps) {
  const sliced = rows.filter((row) => row.total_tokens > 0).slice(0, MAX_SLICES);
  const total = sliced.reduce((sum, row) => sum + row.total_tokens, 0);
  if (total <= 0) {
    return <div className="usage-empty">{t("No model data", "暂无模型数据")}</div>;
  }
  // 1. 从 12 点方向顺时针铺开各扇区
  let angle = -Math.PI / 2;
  const arcs = sliced.map((row, index) => {
    const start = angle;
    angle += (row.total_tokens / total) * Math.PI * 2;
    return {
      row,
      color: SERIES_COLORS[index % SERIES_COLORS.length],
      path: donutPath(start, angle),
    };
  });
  return (
    <div className="usage-donut-wrap">
      <svg viewBox="0 0 200 200" className="usage-donut" role="img" aria-label={t("model distribution", "模型分布")}>
        {arcs.map((arc) => (
          <path key={arc.row.id} d={arc.path} fill={arc.color} />
        ))}
        <text x={CENTER} y="96" textAnchor="middle" className="usage-donut-label">{t("Total", "总计")}</text>
        <text x={CENTER} y="114" textAnchor="middle" className="usage-donut-value">{formatTokens(total)}</text>
      </svg>
      <ul className="usage-donut-legend">
        {arcs.map((arc) => (
          <li key={arc.row.id}>
            <i style={{ background: arc.color }} />
            <span>{arc.row.label}</span>
            <strong>{formatTokens(arc.row.total_tokens)}</strong>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * 生成环形扇区路径。
 *
 * @param start 起始弧度
 * @param end 结束弧度
 * @returns SVG path 指令串
 */
function donutPath(start: number, end: number): string {
  const point = (radius: number, radian: number) =>
    `${CENTER + radius * Math.cos(radian)} ${CENTER + radius * Math.sin(radian)}`;
  // 1. 整圆无法用单段圆弧表示，拆成两段半圆并挖去内圆
  if (end - start >= Math.PI * 2 - 1e-4) {
    return [
      `M ${CENTER} ${CENTER - RADIUS_OUTER}`,
      `A ${RADIUS_OUTER} ${RADIUS_OUTER} 0 1 1 ${CENTER} ${CENTER + RADIUS_OUTER}`,
      `A ${RADIUS_OUTER} ${RADIUS_OUTER} 0 1 1 ${CENTER} ${CENTER - RADIUS_OUTER}`,
      `M ${CENTER} ${CENTER - RADIUS_INNER}`,
      `A ${RADIUS_INNER} ${RADIUS_INNER} 0 1 0 ${CENTER} ${CENTER + RADIUS_INNER}`,
      `A ${RADIUS_INNER} ${RADIUS_INNER} 0 1 0 ${CENTER} ${CENTER - RADIUS_INNER}`,
      "Z",
    ].join(" ");
  }
  // 2. 普通扇区：外弧顺时针、内弧逆时针闭合
  const large = end - start > Math.PI ? 1 : 0;
  return [
    `M ${point(RADIUS_OUTER, start)}`,
    `A ${RADIUS_OUTER} ${RADIUS_OUTER} 0 ${large} 1 ${point(RADIUS_OUTER, end)}`,
    `L ${point(RADIUS_INNER, end)}`,
    `A ${RADIUS_INNER} ${RADIUS_INNER} 0 ${large} 0 ${point(RADIUS_INNER, start)}`,
    "Z",
  ].join(" ");
}
