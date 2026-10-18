export type DisplayScale = {
  /** 界面字号相对 16px 的百分比 */
  fontScale: number;
  /** 页面缩放百分比 */
  pageZoom: number;
};

const STORAGE_KEY = "sai.display-scale";
const FONT_STEPS = [87.5, 100, 112.5, 125] as const;
const ZOOM_STEPS = [90, 100, 110, 125] as const;

/**
 * 【外观设置】【显示尺度】读取已保存的字号与页面缩放。
 * @returns 合法的字号和缩放百分比
 */
export function loadDisplayScale(): DisplayScale {
  try {
    const stored = JSON.parse(globalThis.localStorage?.getItem(STORAGE_KEY) ?? "") as Partial<DisplayScale>;
    return {
      fontScale: FONT_STEPS.includes(stored.fontScale as typeof FONT_STEPS[number]) ? stored.fontScale as number : 100,
      pageZoom: ZOOM_STEPS.includes(stored.pageZoom as typeof ZOOM_STEPS[number]) ? stored.pageZoom as number : 100
    };
  } catch {
    return { fontScale: 100, pageZoom: 100 };
  }
}

/**
 * 【外观设置】【显示尺度】把字号写到 html，页面缩放写到根节点。
 * @param scale 字号和缩放百分比
 * @returns 无
 */
export function applyDisplayScale(scale: DisplayScale): void {
  const root = globalThis.document?.documentElement;
  if (!root) return;
  root.style.fontSize = `${scale.fontScale}%`;
  root.style.zoom = scale.pageZoom === 100 ? "" : `${scale.pageZoom}%`;
}

/**
 * 【外观设置】【显示尺度】保存并立即应用到当前文档。
 * @param scale 字号和缩放百分比
 * @returns 无
 */
export function saveDisplayScale(scale: DisplayScale): void {
  globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(scale));
  applyDisplayScale(scale);
}

export const FONT_SCALE_OPTIONS = FONT_STEPS.map((value) => ({ value: String(value), label: `${value}%` }));
export const PAGE_ZOOM_OPTIONS = ZOOM_STEPS.map((value) => ({ value: String(value), label: `${value}%` }));

/**
 * 解析根节点 zoom 的计算值。
 *
 * @param raw getComputedStyle 读到的 zoom，可能是 1.25 或 125%
 * @returns 大于 0 的倍数；无法解析时为 1
 */
export function pageZoomFactor(raw: string | null | undefined): number {
  const text = raw?.trim() ?? "";
  if (!text || text === "normal") return 1;
  const value = Number.parseFloat(text);
  if (!Number.isFinite(value) || value <= 0) return 1;
  return text.endsWith("%") ? value / 100 : value;
}

/**
 * 读取当前文档的页面缩放倍数。
 *
 * @returns html zoom；没有文档时为 1
 */
export function rootPageZoom(): number {
  const root = globalThis.document?.documentElement;
  if (!root) return 1;
  return pageZoomFactor(getComputedStyle(root).zoom);
}

/**
 * 把指针视口坐标换成 position:fixed 的 CSS 像素。
 *
 * clientX 已是缩放后的视口像素，fixed 的 left/top 还会再乘一次 zoom。
 *
 * @param clientX 指针横坐标
 * @param clientY 指针纵坐标
 * @param zoom 页面缩放倍数
 * @returns 不受二次缩放影响的坐标
 */
export function fixedClientPoint(clientX: number, clientY: number, zoom = rootPageZoom()): { x: number; y: number } {
  const factor = zoom > 0 ? zoom : 1;
  return { x: clientX / factor, y: clientY / factor };
}

/**
 * 返回与 fixed 定位同一坐标系的视口尺寸。
 *
 * @param zoom 页面缩放倍数
 * @returns CSS 像素下的视口宽高
 */
export function fixedViewportBox(zoom = rootPageZoom()): { width: number; height: number } {
  const factor = zoom > 0 ? zoom : 1;
  return {
    width: (globalThis.innerWidth || 0) / factor,
    height: (globalThis.innerHeight || 0) / factor
  };
}
