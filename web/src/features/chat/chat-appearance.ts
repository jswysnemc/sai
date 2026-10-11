import { useCallback, useSyncExternalStore } from "react";

export const CHAT_APPEARANCE_STORAGE_KEY = "sai.chat-appearance";
export const CHAT_APPEARANCE_VERSION = 1;

export type ChatFont = "system" | "sans" | "serif" | "mono";
export type ChatWidth = "standard" | "wide" | "full";

export type ChatAppearance = {
  font: ChatFont;
  fontSize: number;
  width: ChatWidth;
};

export type AppearanceStorage = Pick<Storage, "getItem" | "setItem">;

/**
 * 标准列宽取 50rem：当前工作台默认 52rem 含边距偏宽，阅读列收一档后与正文 15px 更匹配。
 * 宽栏 75rem，铺满 100%。字号默认 15px，范围 12–24，只作用于聊天阅读列，不改界面根字号。
 */
export const DEFAULT_CHAT_APPEARANCE: ChatAppearance = {
  font: "system",
  fontSize: 15,
  width: "standard"
};

const FONT_STACKS: Record<ChatFont, string> = {
  system: 'var(--font-ui)',
  sans: 'Inter, "Noto Sans SC", Arial, "PingFang SC", "Microsoft YaHei UI", sans-serif',
  serif: 'Georgia, "Songti SC", "Source Han Serif SC", "Noto Serif CJK SC", SimSun, serif',
  mono: 'var(--font-code)'
};

const WIDTHS: Record<ChatWidth, string> = {
  standard: "50rem",
  wide: "75rem",
  full: "100%"
};

const FONTS: readonly ChatFont[] = ["system", "sans", "serif", "mono"];
const WIDTH_VALUES: readonly ChatWidth[] = ["standard", "wide", "full"];

let currentSnapshot: ChatAppearance | null = null;
const listeners = new Set<() => void>();

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * 把未知值夹紧为可落地的聊天外观。
 *
 * @param value 本地存储或调用方传入的对象
 * @returns 合法的字体、字号与列宽
 */
export function normalizeChatAppearance(value: unknown): ChatAppearance {
  const record = isRecord(value) ? value : {};
  const fontSize = typeof record.fontSize === "number" && Number.isFinite(record.fontSize)
    ? Math.round(Math.min(24, Math.max(12, record.fontSize)))
    : DEFAULT_CHAT_APPEARANCE.fontSize;
  return {
    font: FONTS.includes(record.font as ChatFont) ? record.font as ChatFont : "system",
    fontSize,
    width: WIDTH_VALUES.includes(record.width as ChatWidth) ? record.width as ChatWidth : "standard"
  };
}

/**
 * 读取可用的本地存储。
 *
 * @returns localStorage；不可用时为 null
 */
export function getAppearanceStorage(): AppearanceStorage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function parseStored(raw: string | null): Record<string, unknown> | null {
  if (!raw) return null;
  try {
    const value: unknown = JSON.parse(raw);
    return isRecord(value) ? value : null;
  } catch {
    return null;
  }
}

function isFutureVersion(value: Record<string, unknown> | null): boolean {
  return value?.version !== undefined && value.version !== CHAT_APPEARANCE_VERSION;
}

/**
 * 读取已保存的聊天外观；损坏、未来版本或不可用时回落默认。
 *
 * @param storage 可选存储，测试可注入
 * @returns 归一化后的外观
 */
export function readChatAppearance(storage: AppearanceStorage | null = getAppearanceStorage()): ChatAppearance {
  try {
    const value = parseStored(storage?.getItem(CHAT_APPEARANCE_STORAGE_KEY) ?? null);
    return normalizeChatAppearance(isFutureVersion(value) ? null : value);
  } catch {
    return { ...DEFAULT_CHAT_APPEARANCE };
  }
}

/**
 * 写入聊天外观，保留同版本未知字段，不覆盖未来版本。
 *
 * @param appearance 待保存的外观
 * @param storage 可选存储
 * @returns 无
 */
export function persistChatAppearance(
  appearance: ChatAppearance,
  storage: AppearanceStorage | null = getAppearanceStorage()
): void {
  if (!storage) return;
  try {
    const value = parseStored(storage.getItem(CHAT_APPEARANCE_STORAGE_KEY));
    if (isFutureVersion(value)) return;
    storage.setItem(
      CHAT_APPEARANCE_STORAGE_KEY,
      JSON.stringify({ ...value, version: CHAT_APPEARANCE_VERSION, ...normalizeChatAppearance(appearance) })
    );
  } catch {
    // 存储被禁用时控件仍可即时预览
  }
}

/**
 * 把外观换成可挂到节点上的 CSS 变量。
 *
 * @param appearance 当前外观
 * @returns 字体、字号与列宽变量
 */
export function getChatAppearanceStyle(appearance: ChatAppearance): Record<string, string> {
  const normalized = normalizeChatAppearance(appearance);
  return {
    "--chat-font-family": FONT_STACKS[normalized.font],
    "--chat-font-size": `${normalized.fontSize}px`,
    "--chat-column-width": WIDTHS[normalized.width]
  };
}

/**
 * 把外观写到文档根节点，聊天列立即跟随。
 *
 * @param appearance 当前外观
 * @returns 无
 */
export function applyChatAppearance(appearance: ChatAppearance): void {
  if (typeof document === "undefined") return;
  for (const [name, value] of Object.entries(getChatAppearanceStyle(appearance))) {
    document.documentElement.style.setProperty(name, value);
  }
}

function emitChange(): void {
  listeners.forEach((listener) => listener());
}

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

function getSnapshot(): ChatAppearance {
  if (currentSnapshot) return currentSnapshot;
  currentSnapshot = readChatAppearance();
  return currentSnapshot;
}

function handleStorageChange(event: StorageEvent): void {
  if (event.key !== CHAT_APPEARANCE_STORAGE_KEY && event.key !== null) return;
  currentSnapshot = readChatAppearance();
  applyChatAppearance(currentSnapshot);
  emitChange();
}

function commit(appearance: ChatAppearance): void {
  const next = normalizeChatAppearance(appearance);
  persistChatAppearance(next);
  applyChatAppearance(next);
  currentSnapshot = next;
  emitChange();
}

/**
 * 在 React 渲染前应用已保存的聊天列外观。
 *
 * @returns 无
 */
export function initializeChatAppearance(): void {
  const appearance = readChatAppearance();
  currentSnapshot = appearance;
  applyChatAppearance(appearance);
}

/**
 * 订阅当前浏览器的聊天阅读外观。
 *
 * @returns 当前外观、局部更新与重置
 */
export function useChatAppearance() {
  const appearance = useSyncExternalStore(subscribe, getSnapshot, () => DEFAULT_CHAT_APPEARANCE);

  const setAppearance = useCallback((patch: Partial<ChatAppearance>) => {
    commit({ ...getSnapshot(), ...patch });
  }, []);

  const resetAppearance = useCallback(() => {
    commit(DEFAULT_CHAT_APPEARANCE);
  }, []);

  return { appearance, setAppearance, resetAppearance };
}
