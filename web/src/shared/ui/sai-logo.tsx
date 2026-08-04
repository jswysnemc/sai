type SaiLogoProps = {
  size?: number;
};

/**
 * 品牌标志的单位网格：5 行 7 列，1 表示实心块。
 *
 * 形态为双箭头（快进式提示符）加一枚落在基线上的实心光标块，
 * 取代旧的像素字母 SAI。与 TUI 端 `brand_logo.rs` 共用同一套网格。
 */
const LOGO_GRID = [
  [1, 0, 1, 0, 0, 0, 0],
  [0, 1, 0, 1, 0, 0, 0],
  [0, 0, 1, 0, 1, 0, 0],
  [0, 1, 0, 1, 0, 1, 1],
  [1, 0, 1, 0, 0, 1, 1]
] as const;

/** 单位网格边长，7 列 * 3.5 = 24.5，在方形图标内保留克制留白 */
const UNIT = 3.5;
/** 网格左上角在 viewBox 中的原点，使字身水平垂直居中 */
const ORIGIN_X = 3.75;
const ORIGIN_Y = 7.25;

/**
 * 渲染 Sai 品牌标志：双箭头提示符加基线光标块。
 *
 * 全部笔画沿单位网格对齐，不使用圆角与渐变，
 * 保证 TUI 字符网格能同构还原。
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
