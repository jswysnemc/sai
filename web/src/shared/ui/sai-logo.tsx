type SaiLogoProps = {
  size?: number;
};

/**
 * 渲染 Sai 品牌标志：圆角方块上的几何 S，带轻微猫耳提示。
 *
 * @param props 尺寸（像素，默认 20）
 * @returns 品牌 SVG 图标
 */
export function SaiLogo({ size = 20 }: SaiLogoProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" role="img" aria-label="Sai">
      <defs>
        <linearGradient id="sai-mark" x1="6" y1="4" x2="28" y2="30" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="color-mix(in srgb, var(--signal, #477d70) 88%, #ffffff)" />
          <stop offset="100%" stopColor="var(--signal, #477d70)" />
        </linearGradient>
      </defs>
      <path
        d="M8.2 4.2 L12.4 1.8 L16 5.1 L19.6 1.8 L23.8 4.2 L28 8.6 V24.8 C28 27 26.2 28.8 24 28.8 H8 C5.8 28.8 4 27 4 24.8 V8.6 Z"
        fill="url(#sai-mark)"
      />
      <path
        d="M22.2 10.2 C20.8 8.7 18.8 8 16.3 8 C12.8 8 10.1 9.7 10.1 12.4 C10.1 15 12.4 16 16.4 16.7 C20.2 17.3 22 18.4 22 20.8 C22 23.5 19.4 25 16 25 C13.2 25 10.9 24.1 9.5 22.4"
        fill="none"
        stroke="#f7fbf9"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="24.7" cy="8.2" r="1.35" fill="#f7fbf9" opacity="0.92" />
    </svg>
  );
}
