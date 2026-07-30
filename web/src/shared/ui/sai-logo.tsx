type SaiLogoProps = {
  size?: number;
};

/**
 * 品牌标志的单位网格：5 行 11 列，1 表示实心块。
 *
 * 依次为字母 S、A、I（各占 3、3、1 列，字母间留 1 列间隙），
 * 末列是与字身分离的方形光标。与 TUI 端 `brand_logo.rs` 共用同一套网格。
 */
const LOGO_GRID = [
  [1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0],
  [1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0],
  [1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0],
  [0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
  [1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1]
] as const;

/** 单位网格边长，11 列 * 2.8 = 30.8，留出左右边距后落在 32 的 viewBox 内 */
const UNIT = 2.8;
/** 网格左上角在 viewBox 中的原点，使字身水平垂直居中 */
const ORIGIN_X = 0.6;
const ORIGIN_Y = 9;

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
