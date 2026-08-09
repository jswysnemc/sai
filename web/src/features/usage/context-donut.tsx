/** 环形图周长基准：r = 100 / 2π，弧长直接按百分数书写 */
const CIRCUMFERENCE = 100;
const RADIUS = 15.915;

export type ContextDonutSegment = {
  /** 稳定渲染键 */
  key: string;
  /** 扇区颜色 */
  color: string;
  /** 相对份额（0~1，各段总和为 1） */
  share: number;
  /** 悬浮提示文本 */
  title: string;
};

/**
 * 渲染上下文构成环形图。
 *
 * 扇区按分项的相对份额划分——分母是分项总和而非整个上下文窗口，
 * 整体占用极低时各分项依然清晰可辨；总占用与用量放在环心。
 *
 * @param props 分段数据、环心主文案与说明、无障碍标签
 * @returns 环形图
 */
export function ContextDonut({
  segments,
  percentLabel,
  usedLabel,
  ariaLabel
}: {
  segments: ContextDonutSegment[];
  percentLabel: string;
  usedLabel: string;
  ariaLabel: string;
}) {
  const arcs = donutArcs(segments.map((segment) => segment.share));
  return (
    <div className="context-donut" role="img" aria-label={ariaLabel}>
      <svg viewBox="0 0 42 42" aria-hidden="true">
        <circle className="context-donut-track" cx="21" cy="21" r={RADIUS} />
        {segments.map((segment, index) => (
          arcs[index].length > 0 && (
            <circle
              key={segment.key}
              cx="21"
              cy="21"
              r={RADIUS}
              stroke={segment.color}
              strokeDasharray={`${arcs[index].length} ${CIRCUMFERENCE - arcs[index].length}`}
              strokeDashoffset={arcs[index].offset}
            >
              <title>{segment.title}</title>
            </circle>
          )
        ))}
      </svg>
      <div className="context-donut-center">
        <strong>{percentLabel}</strong>
        <small>{usedLabel}</small>
      </div>
    </div>
  );
}

/**
 * 计算各扇区的弧长与起点偏移。
 *
 * 步骤:
 * 1. 份额换算为周长百分数弧长
 * 2. 偏移取 25 减去已累计弧长——SVG 描边从 3 点钟起，回退 25 让首段落在 12 点，
 *    后续段依次顺时针衔接
 *
 * 参数:
 * - `shares`: 各段相对份额（0~1）
 *
 * 返回:
 * - 与输入等长的弧长与偏移数组
 */
export function donutArcs(shares: number[]): Array<{ length: number; offset: number }> {
  let consumed = 0;
  return shares.map((share) => {
    const length = Math.max(0, Math.min(1, share)) * CIRCUMFERENCE;
    const offset = 25 - consumed;
    consumed += length;
    return { length, offset };
  });
}
