type SaiLogoProps = {
  size?: number;
  /**
   * 裁剪到字身：去掉方形留白，把字母标当作字标（wordmark）使用。
   * 此时 size 表示字身宽度，高度约为 size 的 0.51 倍。
   */
  trim?: boolean;
};

/**
 * 品牌标志的字符网格：半块字符（▀ ▄ █）拼出的 "Sai" 字母标。
 *
 * 大写 S 全高，小写 a/i 取 x 字高，i 带点与衬线基座。
 * 与 TUI 端 `brand_logo.rs` 共用同一套文本行。
 */
const LOGO_LINES = [
  "▄▀▀▀▀▄        ▄  ",
  "▀▄▄▄▄   ▀▀▀▀▄ ▄▄ ",
  "     █ ▄▀▀▀▀█  █ ",
  "▀▄▄▄▄▀ ▀▄▄▄██ ▄█▄"
] as const;

/** 半格边长，取 1.5 保证所有坐标为 0.25 的整数倍，避免浮点尾差 */
const UNIT = 1.5;
/** 一个字符行的高度（上下两个半格） */
const ROW_HEIGHT = UNIT * 2;
/** 网格左上角在 viewBox 中的原点，使字身水平垂直居中 */
const ORIGIN_X = 3.25;
const ORIGIN_Y = 10;

type LogoRect = { x: number; y: number; width: number; height: number };

/** 将字符网格展开为矩形列表：█ 全格，▀ 上半格，▄ 下半格 */
const LOGO_RECTS: LogoRect[] = LOGO_LINES.flatMap((line, rowIndex) =>
  [...line].flatMap((cell, columnIndex) => {
    const x = ORIGIN_X + columnIndex * UNIT;
    const y = ORIGIN_Y + rowIndex * ROW_HEIGHT;
    if (cell === "█") {
      return [{ x, y, width: UNIT, height: ROW_HEIGHT }];
    }
    if (cell === "▀") {
      return [{ x, y, width: UNIT, height: UNIT }];
    }
    if (cell === "▄") {
      return [{ x, y: y + UNIT, width: UNIT, height: UNIT }];
    }
    return [];
  })
);

/**
 * 渲染 Sai 品牌标志："Sai" 半块字符字母标。
 *
 * 全部笔画沿半格网格对齐的几何矩形，不使用字体渲染与渐变，
 * 保证浏览器与 TUI 字符网格同构还原、边缘无缝。
 * trim 模式下 viewBox 收紧到字身（字身区域 x 3.25–28.75、y 10–22），
 * 适合侧栏品牌位等需要字标而非方形图标的场景。
 *
 * @param props 尺寸（像素，默认 20）与裁剪开关
 * @returns 品牌 SVG 图标
 */
export function SaiLogo({ size = 20, trim = false }: SaiLogoProps) {
  // 字身 25.5 × 12，四周各留 0.75 呼吸边距
  const viewBox = trim ? "2.5 9.25 27 13.5" : "0 0 32 32";
  const width = size;
  const height = trim ? size * 0.5 : size;
  return (
    <svg width={width} height={height} viewBox={viewBox} role="img" aria-label="Sai">
      <g fill="var(--signal, #3a7264)" shapeRendering="crispEdges">
        {LOGO_RECTS.map((rect, index) => (
          <rect
            key={index}
            x={rect.x}
            y={rect.y}
            width={rect.width}
            height={rect.height}
          />
        ))}
      </g>
    </svg>
  );
}
