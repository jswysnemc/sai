type SaiLogoProps = {
  size?: number;
  /** 显示完整字标；size 为字标宽度 */
  trim?: boolean;
};

/** 品牌绿圆点：图标中位于 S 右上方，字标中兼作 i 的点。 */
const DOT_COLOR = "var(--signal, #3a7264)";

/**
 * 渲染 Sai 网页标志：单笔画圆角 S 加品牌绿圆点，与终端启动区的标志同形。
 * @param props 图标尺寸及完整字标开关
 * @returns 笔画随主题文字颜色变化、圆点固定品牌色的矢量标志
 */
export function SaiLogo({ size = 20, trim = false }: SaiLogoProps) {
  if (trim) {
    return <svg width={size} height={size / 2} viewBox="3.5 3 48 24" role="img" aria-label="Sai">
      <g fill="none" stroke="currentColor" strokeWidth="3.25" strokeLinecap="round" strokeLinejoin="round">
        {/* s：与图标同一条笔画，按小写 x 高度收紧 */}
        <path d="M20 11H11a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H7" />
        {/* a：闭合圆腹 + 右侧竖干 */}
        <path d="M40 11v14m0-7a6 6 0 1 0-12 0 6 6 0 0 0 12 0" />
        {/* i：竖干，点由品牌绿圆点承担 */}
        <path d="M48 13v12" />
      </g>
      <circle cx="48" cy="6" r="2.5" fill={DOT_COLOR} />
    </svg>;
  }
  return <svg width={size} height={size} viewBox="0 0 32 32" role="img" aria-label="Sai">
    <path d="M21 8H11a4.5 4.5 0 0 0 0 9h6a4.5 4.5 0 0 1 0 9H7" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" />
    <circle cx="27" cy="7" r="2.5" fill={DOT_COLOR} />
  </svg>;
}
