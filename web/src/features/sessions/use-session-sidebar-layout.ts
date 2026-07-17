import { useEffect, useState } from "react";

export const SESSION_SIDEBAR_MIN_WIDTH = 190;
export const SESSION_SIDEBAR_MAX_WIDTH = 420;
export const SESSION_SIDEBAR_DEFAULT_WIDTH = 232;

const COLLAPSED_STORAGE_KEY = "sai.session-sidebar-collapsed";
const WIDTH_STORAGE_KEY = "sai.session-sidebar-width";

/**
 * 把会话侧栏宽度限制在可用范围内。
 *
 * @param width 请求设置的侧栏宽度
 * @returns 限制后的侧栏宽度
 */
export function clampSessionSidebarWidth(width: number): number {
  return Math.min(SESSION_SIDEBAR_MAX_WIDTH, Math.max(SESSION_SIDEBAR_MIN_WIDTH, width));
}

/**
 * 解析本地保存的会话侧栏宽度。
 *
 * @param raw 本地存储中的原始值
 * @returns 可用的侧栏宽度
 */
export function parseSessionSidebarWidth(raw: string | null): number {
  if (raw === null) return SESSION_SIDEBAR_DEFAULT_WIDTH;
  const width = Number(raw);
  return Number.isFinite(width) ? clampSessionSidebarWidth(width) : SESSION_SIDEBAR_DEFAULT_WIDTH;
}

/**
 * 管理会话侧栏的折叠状态、宽度和本地持久化。
 *
 * @returns 会话侧栏布局状态与操作方法
 */
export function useSessionSidebarLayout() {
  const [collapsed, setCollapsed] = useState(() => window.localStorage.getItem(COLLAPSED_STORAGE_KEY) === "true");
  const [width, setWidth] = useState(() => parseSessionSidebarWidth(window.localStorage.getItem(WIDTH_STORAGE_KEY)));

  useEffect(() => {
    window.localStorage.setItem(COLLAPSED_STORAGE_KEY, String(collapsed));
  }, [collapsed]);

  useEffect(() => {
    window.localStorage.setItem(WIDTH_STORAGE_KEY, String(width));
  }, [width]);

  /** 切换会话侧栏的折叠状态。 */
  const toggleCollapsed = () => setCollapsed((current) => !current);

  /** 展开会话侧栏。 */
  const expand = () => setCollapsed(false);

  /**
   * 更新会话侧栏宽度。
   *
   * @param nextWidth 请求设置的侧栏宽度
   */
  const resize = (nextWidth: number) => setWidth(clampSessionSidebarWidth(nextWidth));

  return { collapsed, width, toggleCollapsed, expand, resize };
}
