import { useCallback, useState } from "react";
import { browserViewportSize, type BrowserViewportSize } from "./browser-viewport-size";

/** 缩放档位：fit 表示按面板大小自动缩放。 */
export type BrowserZoom = "fit" | 0.5 | 0.75 | 1;

/** 自由尺寸模式的常用设备尺寸。 */
export const RESPONSIVE_PRESETS: { id: string; labelEn: string; labelZh: string; width: number; height: number }[] = [
  { id: "desktop", labelEn: "Desktop", labelZh: "桌面", width: 1440, height: 900 },
  { id: "laptop", labelEn: "Laptop", labelZh: "笔记本", width: 1280, height: 800 },
  { id: "tablet", labelEn: "Tablet", labelZh: "平板", width: 820, height: 1180 },
  { id: "phone", labelEn: "Phone", labelZh: "手机", width: 390, height: 844 }
];

/** 可选缩放档位。 */
export const ZOOM_OPTIONS: BrowserZoom[] = ["fit", 1, 0.75, 0.5];

/** 自由尺寸状态。 */
export type ResponsiveViewport = { size: BrowserViewportSize; zoom: BrowserZoom };

/**
 * 【浏览器面板】【自由尺寸】计算画面实际显示比例：fit 时按面板可用区域等比缩小，不放大。
 *
 * @param page 页面视口尺寸
 * @param host 面板可用尺寸
 * @param zoom 缩放档位
 * @returns 显示比例
 */
export function displayScale(page: BrowserViewportSize, host: BrowserViewportSize | null, zoom: BrowserZoom): number {
  if (zoom !== "fit") return zoom;
  if (!host || host.width <= 0 || host.height <= 0) return 1;
  return Math.min(1, host.width / page.width, host.height / page.height);
}

/**
 * 管理自由尺寸模式：开启后页面视口按指定宽高渲染，不再跟随面板大小。
 *
 * @returns 当前状态与切换、调整方法
 */
export function useResponsiveViewport() {
  const [viewport, setViewport] = useState<ResponsiveViewport | null>(null);

  /** 开启或关闭自由尺寸；开启时默认使用手机尺寸并自动缩放。 */
  const toggle = useCallback(() => {
    setViewport((current) => current ? null : { size: { width: 390, height: 844 }, zoom: "fit" });
  }, []);

  /** 修改宽高，按服务端限制裁剪。 */
  const setSize = useCallback((size: BrowserViewportSize) => {
    const limited = browserViewportSize(size);
    if (!limited) return;
    setViewport((current) => current ? { ...current, size: limited } : current);
  }, []);

  /** 修改缩放档位。 */
  const setZoom = useCallback((zoom: BrowserZoom) => {
    setViewport((current) => current ? { ...current, zoom } : current);
  }, []);

  return { viewport, toggle, setSize, setZoom };
}
