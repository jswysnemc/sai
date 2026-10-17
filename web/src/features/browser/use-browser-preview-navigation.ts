import { useEffect } from "react";
import {
  browserNavigationMessage,
  consumePendingBrowserNavigation,
  subscribeBrowserNavigation
} from "./browser-preview-navigation";
import type { BrowserClientMessage } from "./browser-protocol";
import type { BrowserConnectionStatus } from "./use-browser-session";

/**
 * 连接成功后打开等待中的 HTML 预览，并继续接收新的打开请求。
 *
 * @param status 浏览器连接状态
 * @param send 面板消息发送方法
 * @returns 无返回值
 */
export function useBrowserPreviewNavigation(
  status: BrowserConnectionStatus,
  send: (message: BrowserClientMessage) => void
): void {
  useEffect(() => {
    if (status !== "connected") return;
    // 1. 先发出打开面板之前就请求的地址
    const queued = consumePendingBrowserNavigation();
    if (queued) send(browserNavigationMessage(queued));
    // 2. 面板保持打开时，后续右键直接导航
    return subscribeBrowserNavigation((url) => send(browserNavigationMessage(url)));
  }, [send, status]);
}
