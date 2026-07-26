type SaiLogoProps = {
  size?: number;
};

/**
 * 渲染 Sai 品牌标志：圆角方块上的双弧几何 S，右上角带一枚终端块光标。
 *
 * S 由两段半圆弧与三段横线构成，端点全部落在 0.1 精度网格上，
 * 小尺寸（18px）下笔画依然清晰；块光标呼应产品的 agent 属性。
 *
 * @param props 尺寸（像素，默认 20）
 * @returns 品牌 SVG 图标
 */
export function SaiLogo({ size = 20 }: SaiLogoProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" role="img" aria-label="Sai">
      <defs>
        <linearGradient id="sai-mark" x1="4" y1="4" x2="28" y2="30" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="color-mix(in srgb, var(--signal, #3a7264) 84%, #ffffff)" />
          <stop offset="100%" stopColor="var(--signal, #3a7264)" />
        </linearGradient>
      </defs>
      <rect x="1" y="1" width="30" height="30" rx="8.5" fill="url(#sai-mark)" />
      <path
        d="M22.5 8.8 H13.2 a3.6 3.6 0 0 0 0 7.2 H19 a3.6 3.6 0 0 1 0 7.2 H9.5"
        fill="none"
        stroke="#f7fbf9"
        strokeWidth="3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <rect x="24.9" y="7.3" width="3" height="3" rx="0.9" fill="#f7fbf9" opacity="0.95" />
    </svg>
  );
}
