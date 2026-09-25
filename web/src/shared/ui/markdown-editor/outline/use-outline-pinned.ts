import { useSyncExternalStore } from "react";

/** 大纲固定状态的存储键。 */
const STORAGE_KEY = "sai.markdown-outline.pinned";

/** 同页多个编辑器共享同一份固定状态，变化时逐个通知。 */
const listeners = new Set<() => void>();

/**
 * 读取大纲是否固定展开。
 *
 * @returns 固定时为 true；缺省固定，宽度不足时由样式退化为右缘轨道
 */
function readPinned(): boolean {
  try {
    return globalThis.localStorage?.getItem(STORAGE_KEY) !== "off";
  } catch {
    return true;
  }
}

/**
 * 写入大纲固定状态并通知订阅方。
 *
 * @param pinned 是否固定
 * @returns 无
 */
export function writeOutlinePinned(pinned: boolean): void {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, pinned ? "on" : "off");
  } catch {
    // 存储不可用时只在本次会话内生效
  }
  listeners.forEach((listener) => listener());
}

/**
 * 订阅固定状态变化，同时响应其他标签页的存储事件。
 *
 * @param listener 变化回调
 * @returns 取消订阅函数
 */
function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  const onStorage = (event: StorageEvent) => {
    if (event.key === STORAGE_KEY) listener();
  };
  globalThis.addEventListener?.("storage", onStorage);
  return () => {
    listeners.delete(listener);
    globalThis.removeEventListener?.("storage", onStorage);
  };
}

/**
 * 读取并订阅大纲固定状态。
 *
 * @returns 当前是否固定
 */
export function useOutlinePinned(): boolean {
  return useSyncExternalStore(subscribe, readPinned, () => true);
}
