type SaiLogoProps = {
  size?: number;
  /** 显示完整字标；size 为字标宽度 */
  trim?: boolean;
};

/**
 * 渲染 Sai 网页标志，使用连续圆角笔画保持小尺寸辨识度。
 * @param props 图标尺寸及完整字标开关
 * @returns 随主题文字颜色变化的矢量标志
 */
export function SaiLogo({ size = 20, trim = false }: SaiLogoProps) {
  return <svg width={size} height={trim ? size / 2 : size} viewBox={trim ? "0 0 64 32" : "0 0 32 32"} role="img" aria-label="Sai">
    <g fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M24 7H13a4.5 4.5 0 0 0 0 9h6a4.5 4.5 0 0 1 0 9H8" />
      {trim && <>
        <path d="M48 14v11m0-5.5a5.5 5.5 0 1 0-11 0 5.5 5.5 0 0 0 11 0M58 14v11" />
        <path d="M58 7v.25" strokeWidth="4" />
      </>}
    </g>
  </svg>;
}
