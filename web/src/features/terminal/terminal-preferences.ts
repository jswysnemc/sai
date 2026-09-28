import { useSyncExternalStore } from "react";

export const TERMINAL_FONT_FAMILY = '"Fira Code", "SFMono-Regular", Consolas, "Liberation Mono", Menlo, "Sarasa Mono SC", "Noto Sans Mono CJK SC", monospace';
export const TERMINAL_FONTS = {
  default: TERMINAL_FONT_FAMILY,
  system: 'ui-monospace, "SFMono-Regular", Consolas, "Liberation Mono", "Noto Sans Mono CJK SC", monospace',
  monospace: "monospace"
};
export type TerminalPreferences = { font: keyof typeof TERMINAL_FONTS; fontSizeRem: number; scrollback: number };
export const DEFAULT_TERMINAL_PREFERENCES: TerminalPreferences = { font: "default", fontSizeRem: 0.75, scrollback: 1000 };
const STORAGE_KEY = "sai.terminal.display";
const CHANGE_EVENT = "sai:terminal-display";
let volatileValue: string | null = null;

/**
 * 读取终端偏好原始快照，存储不可用时保留本页面的修改。
 * @returns 稳定的序列化快照
 */
function getSnapshot(): string {
  try { return volatileValue ?? window.localStorage.getItem(STORAGE_KEY) ?? ""; }
  catch { return volatileValue ?? ""; }
}

/**
 * 校验本地终端偏好，拒绝损坏数据与无限回滚容量。
 * @param raw 本地存储文本
 * @returns 规范化后的显示偏好
 */
export function parseTerminalPreferences(raw: string): TerminalPreferences {
  try {
    const value = JSON.parse(raw);
    return {
      font: Object.hasOwn(TERMINAL_FONTS, value?.font) ? value.font : "default",
      fontSizeRem: Number.isFinite(value?.fontSizeRem) ? Math.min(1.5, Math.max(0.625, value.fontSizeRem)) : 0.75,
      scrollback: Number.isFinite(value?.scrollback) ? Math.min(50_000, Math.max(0, Math.trunc(value.scrollback))) : 1000
    };
  } catch { return { ...DEFAULT_TERMINAL_PREFERENCES }; }
}

/** 获取当前显示偏好；无参数，返回已校验的配置。 */
export function getTerminalPreferences(): TerminalPreferences {
  return parseTerminalPreferences(getSnapshot());
}

/**
 * 订阅当前页面及其他标签页的显示偏好变化。
 * @param listener 更新回调
 * @returns 取消订阅函数
 */
export function subscribeTerminalPreferences(listener: () => void): () => void {
  const onStorage = (event: StorageEvent) => {
    if (event.key !== STORAGE_KEY && event.key !== null) return;
    volatileValue = null;
    listener();
  };
  window.addEventListener(CHANGE_EVENT, listener);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener(CHANGE_EVENT, listener);
    window.removeEventListener("storage", onStorage);
  };
}

/**
 * 合并并保存终端偏好，不重建终端会话。
 * @param patch 修改字段
 * @returns 无返回值
 */
export function updateTerminalPreferences(patch: Partial<TerminalPreferences>): void {
  const value = JSON.stringify(parseTerminalPreferences(JSON.stringify({ ...getTerminalPreferences(), ...patch })));
  try {
    window.localStorage.setItem(STORAGE_KEY, value);
    volatileValue = null;
  } catch { volatileValue = value; }
  window.dispatchEvent(new Event(CHANGE_EVENT));
}

/** 订阅终端偏好；无参数，返回当前配置和局部更新方法。 */
export function useTerminalPreferences() {
  const raw = useSyncExternalStore(subscribeTerminalPreferences, getSnapshot, () => "");
  return { preferences: parseTerminalPreferences(raw), update: updateTerminalPreferences };
}
