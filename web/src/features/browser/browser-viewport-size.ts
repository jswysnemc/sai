/** 面板和远端页面的 CSS 像素尺寸。 */
export type BrowserViewportSize = { width: number; height: number };

/**
 * 【浏览器面板】【视口同步】直接使用面板宽高，并遵守服务端的尺寸限制。
 * @param size 面板实际可用尺寸
 * @returns 远端视口尺寸；隐藏、过小或无效的面板返回 null
 */
export function browserViewportSize(size: BrowserViewportSize): BrowserViewportSize | null {
  if (!Number.isFinite(size.width) || !Number.isFinite(size.height) || size.width < 32 || size.height < 32) return null;
  const width = Math.min(3840, Math.max(32, Math.floor(size.width)));
  const height = Math.min(2160, Math.max(32, Math.floor(size.height)));
  return { width, height };
}
