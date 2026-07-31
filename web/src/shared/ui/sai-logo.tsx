type SaiLogoProps = {
  size?: number;
};

/**
 * 品牌标志的单位网格：5 行 11 列，1 表示实心块。
 *
 * 依次为字母 S、A、I，末列是下沉两格的终端光标。A 使用完整顶横与开放字腔，
 * 在小尺寸下仍能与 o 清楚区分。与 TUI 端 `brand_logo.rs` 共用同一套网格。
 */
const LOGO_GRID = [
  [1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0],
  [1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0],
  [1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0],
  [0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
  [1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1]
] as const;

/** 单位网格边长，11 列 * 2.5 = 27.5，在方形图标内保留克制留白 */
const UNIT = 2.5;
/** 网格左上角在 viewBox 中的原点，使字身水平垂直居中 */
const ORIGIN_X = 2.25;
const ORIGIN_Y = 9.75;

/**
 * 渲染 Sai 品牌标志：像素网格上的正交实心字身 SAI，末尾带一枚分离的方形光标。
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
        {LOGO_GRID.flatMap((row, rowIndex) =>
          row.map((cell, columnIndex) =>
            cell === 1 ? (
              <rect
                key={`${rowIndex}-${columnIndex}`}
                x={ORIGIN_X + columnIndex * UNIT}
                y={ORIGIN_Y + rowIndex * UNIT}
                width={UNIT}
                height={UNIT}
              />
            ) : null
          )
        )}
      </g>
    </svg>
  );
}
