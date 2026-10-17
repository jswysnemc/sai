import type { BrowserClientMessage } from "./browser-protocol";

type NavigationListener = (url: string) => void;

/** 面板尚未连上时暂存的预览地址。 */
let pendingUrl: string | null = null;
const listeners = new Set<NavigationListener>();

/**
 * 让内置浏览器打开这个地址。
 *
 * 面板已经连上时立即发送；否则留到连接成功后的第一次发送。
 *
 * @param url 完整的 http(s) 地址
 * @returns 无返回值
 */
export function requestBuiltInBrowserNavigation(url: string): void {
  if (listeners.size > 0) {
    pendingUrl = null;
    for (const listener of listeners) listener(url);
    return;
  }
  pendingUrl = url;
}

/**
 * 取出并清掉等待中的预览地址。
 *
 * @returns 尚未发送的地址；没有时为空
 */
export function consumePendingBrowserNavigation(): string | null {
  const url = pendingUrl;
  pendingUrl = null;
  return url;
}

/**
 * 在浏览器面板连接后接收后续打开请求。
 *
 * @param listener 发送导航的回调
 * @returns 取消订阅
 */
export function subscribeBrowserNavigation(listener: NavigationListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * 把预览地址收成面板导航消息。
 *
 * @param url 完整地址
 * @returns 导航消息
 */
export function browserNavigationMessage(url: string): BrowserClientMessage {
  return { type: "navigate", url };
}
