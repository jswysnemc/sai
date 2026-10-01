/**
 * 【浏览器面板】【像素比限制】限制录屏像素密度，兼顾高分屏清晰度和传输体积。
 * @param ratio 屏幕上报的设备像素比
 * @returns 1 到 2 之间的像素比，保留两位小数；非法值返回 1
 */
export function browserDeviceScale(ratio: number): number {
  if (!Number.isFinite(ratio) || ratio <= 0) return 1;
  return Math.round(Math.min(2, Math.max(1, ratio)) * 100) / 100;
}

/**
 * 【浏览器面板】【像素比监听】监听跨屏移动和浏览器缩放，即使面板 CSS 尺寸未变也通知调用方。
 * @param onChange 首次订阅或有效像素比变化时接收新值
 * @param target 面板所在窗口，默认使用当前窗口
 * @returns 清理全部监听的方法
 */
export function observeBrowserDeviceScale(onChange: (scale: number) => void, target: Window = window): () => void {
  let query: MediaQueryList | null = null;
  let previous: number | null = null;

  /**
   * 【浏览器面板】【像素比更新】重建当前屏幕密度的查询，持续跟踪后续变化。
   * @returns 无
   */
  const update = () => {
    query?.removeEventListener("change", update);
    const raw = target.devicePixelRatio;
    const ratio = Number.isFinite(raw) && raw > 0 ? raw : 1;
    query = target.matchMedia(`(resolution: ${ratio}dppx)`);
    query.addEventListener("change", update);
    const scale = browserDeviceScale(ratio);
    if (scale !== previous) {
      previous = scale;
      onChange(scale);
    }
  };

  // 1. 分辨率查询捕获跨屏变化，窗口事件补充浏览器缩放通知
  target.addEventListener("resize", update);
  update();
  // 2. 组件卸载时移除最新查询与窗口监听
  return () => {
    query?.removeEventListener("change", update);
    target.removeEventListener("resize", update);
  };
}
