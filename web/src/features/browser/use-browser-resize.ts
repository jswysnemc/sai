import { useEffect, useRef, useState, type RefObject } from "react";
import type { BrowserClientMessage } from "./browser-protocol";
import { browserViewportSize, type BrowserViewportSize } from "./browser-viewport-size";
import { browserDeviceScale, observeBrowserDeviceScale } from "./browser-device-scale";

/**
 * 【浏览器面板】【尺寸同步】观察面板大小，在连接就绪或重连后重新通知服务端。
 *
 * 自由尺寸模式下改为发送指定宽高，面板大小只用于计算显示缩放。
 *
 * @param hostRef 面板容器
 * @param connected 服务端会话是否就绪
 * @param onSend 控制消息发送方法
 * @param fixed 自由尺寸模式的页面宽高；为空时跟随面板
 * @returns 面板显示尺寸
 */
export function useBrowserResize(
  hostRef: RefObject<HTMLDivElement | null>,
  connected: boolean,
  onSend: (message: BrowserClientMessage) => void,
  fixed: BrowserViewportSize | null = null
): BrowserViewportSize | null {
  const [size, setSize] = useState<BrowserViewportSize | null>(null);
  const [scale, setScale] = useState(() => browserDeviceScale(window.devicePixelRatio));
  const sendRef = useRef(onSend);
  sendRef.current = onSend;

  useEffect(() => observeBrowserDeviceScale(setScale), []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const observer = new ResizeObserver(([entry]) => {
      const width = Math.floor(entry.contentRect.width);
      const height = Math.floor(entry.contentRect.height);
      setSize(previous => previous?.width === width && previous?.height === height ? previous : { width, height });
    });
    observer.observe(host);
    return () => observer.disconnect();
  }, [hostRef]);

  const fixedWidth = fixed?.width;
  const fixedHeight = fixed?.height;
  useEffect(() => {
    const target = fixedWidth && fixedHeight ? { width: fixedWidth, height: fixedHeight } : size;
    if (!connected || !target) return;
    const viewport = browserViewportSize(target);
    if (!viewport) return;
    const timer = window.setTimeout(() => sendRef.current({ type: "resize", ...viewport, scale }), 150);
    return () => window.clearTimeout(timer);
  }, [connected, size, fixedWidth, fixedHeight, scale]);

  return size;
}
