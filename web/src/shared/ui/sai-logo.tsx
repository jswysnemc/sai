type SaiLogoProps = {
  size?: number;
};

/// 单位网格边长（viewBox 为 32，S 主体占 5 行 4 列，右侧留出分离块与间隙）
const UNIT = 5;
/// S 主体左上角在 viewBox 中的原点
const ORIGIN_X = 1;
const ORIGIN_Y = 3.5;

/**
 * 渲染 Sai 品牌标志：像素网格上的正交实心 S，右下角带一枚分离的方形光标。
 *
 * 全部笔画沿单位网格对齐，笔画与留白同为一个单位，因此在 16px 下
 * 仍保持锐利的直角边缘；不使用圆角与渐变，保证 TUI 字符网格能同构还原。
 *
 * @param props 尺寸（像素，默认 20）
 * @returns 品牌 SVG 图标
 */
export function SaiLogo({ size = 20 }: SaiLogoProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" role="img" aria-label="Sai">
      <g fill="var(--signal, #3a7264)" shapeRendering="crispEdges">
        {/* S 上横：占满 4 列 */}
        <rect x={ORIGIN_X} y={ORIGIN_Y} width={UNIT * 4} height={UNIT} />
        {/* S 左竖：连接上横与中横 */}
        <rect x={ORIGIN_X} y={ORIGIN_Y + UNIT} width={UNIT} height={UNIT} />
        {/* S 中横：占满 4 列 */}
        <rect x={ORIGIN_X} y={ORIGIN_Y + UNIT * 2} width={UNIT * 4} height={UNIT} />
        {/* S 右竖：连接中横与下横 */}
        <rect x={ORIGIN_X + UNIT * 3} y={ORIGIN_Y + UNIT * 3} width={UNIT} height={UNIT} />
        {/* S 下横：占满 4 列 */}
        <rect x={ORIGIN_X} y={ORIGIN_Y + UNIT * 4} width={UNIT * 4} height={UNIT} />
        {/* 分离光标块：与主体隔开一个单位，呼应终端块光标 */}
        <rect
          x={ORIGIN_X + UNIT * 5}
          y={ORIGIN_Y + UNIT * 3}
          width={UNIT}
          height={UNIT * 2}
        />
      </g>
    </svg>
  );
}
