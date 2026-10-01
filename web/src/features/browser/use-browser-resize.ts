import { useEffect, useRef, useState, type RefObject } from "react";
import type { BrowserClientMessage } from "./browser-protocol";
import { browserViewportSize, type BrowserViewportSize } from "./browser-viewport-size";

/**
 * 【浏览器面板】【尺寸同步】观察面板大小，在连接就绪或重连后重新通知服务端。
 * @param hostRef 面板容器
 * @param connected 服务端会话是否就绪
 * @param onSend 控制消息发送方法
 * @returns 面板显示尺寸
 */
export function useBrowserResize(
  hostRef: RefObject<HTMLDivElement | null>,
  connected: boolean,
  onSend: (message: BrowserClientMessage) => void
): BrowserViewportSize | null {
  const [size, setSize] = useState<BrowserViewportSize | null>(null);
  const sendRef = useRef(onSend);
  sendRef.current = onSend;

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

  useEffect(() => {
    if (!connected || !size) return;
    const viewport = browserViewportSize(size);
    if (!viewport) return;
    const timer = window.setTimeout(() => sendRef.current({ type: "resize", ...viewport }), 150);
    return () => window.clearTimeout(timer);
  }, [connected, size]);

  return size;
}
