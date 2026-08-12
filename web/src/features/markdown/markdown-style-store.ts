import { useCallback, useSyncExternalStore } from "react";
import {
  DEFAULT_MARKDOWN_STYLE_PREFERENCES,
  MARKDOWN_STYLE_STORAGE_KEY,
  normalizeMarkdownStylePreferences,
  parseMarkdownStylePreferences,
  type MarkdownCodeBlockStylePreferences,
  type MarkdownStylePreferences,
  type MarkdownStylePreset,
  type MarkdownTableStylePreferences
} from "./markdown-style-preferences";

let currentSnapshot: MarkdownStylePreferences | null = null;
const listeners = new Set<() => void>();

/**
 * 订阅 Markdown 外观配置变化，并在首个订阅者出现时监听跨标签页更新。
 *
 * @param listener 配置变化回调
 * @returns 取消订阅函数
 */
function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  if (listeners.size === 1 && typeof window !== "undefined") {
    window.addEventListener("storage", handleStorageChange);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && typeof window !== "undefined") {
      window.removeEventListener("storage", handleStorageChange);
    }
  };
}

/**
 * 获取浏览器中的当前 Markdown 外观配置快照。
 *
 * @returns 当前配置快照
 */
function getSnapshot(): MarkdownStylePreferences {
  if (currentSnapshot) return currentSnapshot;
  currentSnapshot = readStoredPreferences();
  return currentSnapshot;
}

/**
 * 获取服务端渲染使用的稳定默认快照。
 *
 * @returns 默认配置快照
 */
function getServerSnapshot(): MarkdownStylePreferences {
  return DEFAULT_MARKDOWN_STYLE_PREFERENCES;
}

/**
 * 从浏览器本地存储读取 Markdown 外观配置。
 *
 * @returns 已容错解析的完整配置
 */
function readStoredPreferences(): MarkdownStylePreferences {
  if (typeof window === "undefined") return DEFAULT_MARKDOWN_STYLE_PREFERENCES;
  try {
    return parseMarkdownStylePreferences(window.localStorage.getItem(MARKDOWN_STYLE_STORAGE_KEY));
  } catch {
    return DEFAULT_MARKDOWN_STYLE_PREFERENCES;
  }
}

/**
 * 保存并广播完整的 Markdown 外观配置。
 *
 * @param preferences 新配置
 * @returns 无返回值
 */
function setPreferences(preferences: MarkdownStylePreferences): void {
  currentSnapshot = normalizeMarkdownStylePreferences(preferences);
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(MARKDOWN_STYLE_STORAGE_KEY, JSON.stringify(currentSnapshot));
    } catch {
      // 浏览器禁用本地存储时仍保留当前页面内的即时配置
    }
  }
  emitChange();
}

/**
 * 恢复 Markdown 外观默认值并清理本地覆盖。
 *
 * @returns 无返回值
 */
function resetPreferences(): void {
  currentSnapshot = normalizeMarkdownStylePreferences(DEFAULT_MARKDOWN_STYLE_PREFERENCES);
  if (typeof window !== "undefined") {
    try {
      window.localStorage.removeItem(MARKDOWN_STYLE_STORAGE_KEY);
    } catch {
      // 浏览器禁用本地存储时仍完成当前页面内的重置
    }
  }
  emitChange();
}

/**
 * 通知当前页面中的全部配置订阅者。
 *
 * @returns 无返回值
 */
function emitChange(): void {
  listeners.forEach((listener) => listener());
}

/**
 * 接收其他标签页写入的 Markdown 外观配置。
 *
 * @param event 浏览器存储事件
 * @returns 无返回值
 */
function handleStorageChange(event: StorageEvent): void {
  if (event.key !== MARKDOWN_STYLE_STORAGE_KEY) return;
  currentSnapshot = parseMarkdownStylePreferences(event.newValue);
  emitChange();
}

/**
 * 管理 Markdown 外观配置及其表格、代码块局部更新。
 *
 * @returns 当前配置、局部更新方法和重置方法
 */
export function useMarkdownStylePreferences() {
  const preferences = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);

  const updatePreset = useCallback((preset: MarkdownStylePreset) => {
    setPreferences({ ...getSnapshot(), preset });
  }, []);

  const updateTable = useCallback((patch: Partial<MarkdownTableStylePreferences>) => {
    const current = getSnapshot();
    setPreferences({
      ...current,
      table: { ...current.table, ...patch }
    });
  }, []);

  const updateCodeBlock = useCallback((patch: Partial<MarkdownCodeBlockStylePreferences>) => {
    const current = getSnapshot();
    setPreferences({
      ...current,
      codeBlock: { ...current.codeBlock, ...patch }
    });
  }, []);

  return {
    preferences,
    updatePreset,
    updateTable,
    updateCodeBlock,
    reset: resetPreferences
  };
}
