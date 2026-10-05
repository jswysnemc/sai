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
