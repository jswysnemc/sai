import { useCallback, useEffect, useState } from "react";
import type { DiffLayout } from "../diff-view";

const STORAGE_KEY = "sai.diff-view-options";
const MIN_SPLIT_WIDTH = 640;

/**
 * 管理差异布局和换行偏好，并根据实际容器宽度回退到统一视图。
 * @param defaultLayout 没有保存偏好时使用的布局
 * @returns 容器引用、实际布局、换行设置与更新方法
 */
export function useDiffViewOptions(defaultLayout: DiffLayout = "unified") {
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [preferences, setPreferences] = useState(() => readPreferences(defaultLayout));
  const [width, setWidth] = useState<number | null>(null);

  useEffect(() => {
    if (!container) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(container);
    return () => observer.disconnect();
  }, [container]);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
    } catch {
      // 1. 浏览器限制存储时保留本次页面内的设置
    }
  }, [preferences]);

  /**
   * 更新用户选择的布局，窄栏临时回退时仍保留此偏好。
   * @param layout 用户选择的布局
   * @returns 无返回值
   */
  const setLayout = useCallback((layout: DiffLayout) => setPreferences((current) => ({ ...current, layout })), []);

  /**
   * 更新长代码行的换行设置。
   * @param wrap 是否自动换行
   * @returns 无返回值
   */
  const setWrap = useCallback((wrap: boolean) => setPreferences((current) => ({ ...current, wrap })), []);

  const sideAvailable = width === null || width >= MIN_SPLIT_WIDTH;
  return {
    ref: setContainer,
    layout: sideAvailable ? preferences.layout : "unified" as DiffLayout,
    sideAvailable,
    wrap: preferences.wrap,
    setLayout,
    setWrap,
  };
}

/**
 * 读取合法的本地审阅偏好，服务端渲染或存储不可用时使用默认值。
 * @param defaultLayout 默认差异布局
 * @returns 布局与自动换行设置
 */
function readPreferences(defaultLayout: DiffLayout): { layout: DiffLayout; wrap: boolean } {
  try {
    const value = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? "null");
    return { layout: value?.layout === "side" || value?.layout === "unified" ? value.layout : defaultLayout, wrap: value?.wrap !== false };
  } catch {
    return { layout: defaultLayout, wrap: true };
  }
}
