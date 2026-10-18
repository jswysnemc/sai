import { useCallback, useEffect, useRef, useSyncExternalStore, type RefObject } from "react";

export const FONT_PREFERENCES_STORAGE_KEY = "sai.font-preferences";
export const EDITOR_FONT_SIZE_MIN = 8;
export const EDITOR_FONT_SIZE_MAX = 32;
export const DEFAULT_EDITOR_FONT_SIZE = 13;

export type FontPreferences = {
  /** 界面西文字族；空字符串表示使用内置字族 */
  uiFont: string;
  /** 编辑器西文字族；空字符串表示使用代码字族 */
  editorFont: string;
  /** 中文字族；空字符串表示使用等宽中文字族 */
  cjkFont: string;
  /** 编辑器字号，单位 px */
  editorFontSize: number;
};

export const DEFAULT_FONT_PREFERENCES: FontPreferences = {
  uiFont: "",
  editorFont: "",
  cjkFont: "",
  editorFontSize: DEFAULT_EDITOR_FONT_SIZE
};

const EDITOR_LATIN_FALLBACK = '"Fira Code", ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono"';
const CJK_MONO_FALLBACK = '"Sarasa Mono SC", "Noto Sans Mono CJK SC", "Source Han Mono SC", "Noto Sans Mono CJK", "Noto Sans SC"';

let currentSnapshot: FontPreferences | null = null;
const listeners = new Set<() => void>();

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * 清洗用户填写的字族名称，去掉无法安全放进 CSS 的字符。
 *
 * @param value 原始输入
 * @returns 可用字族；非法时为空字符串
 */
export function sanitizeFontFamily(value: unknown): string {
  if (typeof value !== "string") return "";
  return value.replace(/[^\p{L}\p{N} .,_'"()\-]/gu, "").slice(0, 160);
}

/**
 * 把未知值收成可保存的字体偏好。
 *
 * @param value 本地存储或调用方传入的对象
 * @returns 界面字族、编辑器字族与编辑器字号
 */
export function normalizeFontPreferences(value: unknown): FontPreferences {
  const record = isRecord(value) ? value : {};
  const size = typeof record.editorFontSize === "number" && Number.isFinite(record.editorFontSize)
    ? Math.round(Math.min(EDITOR_FONT_SIZE_MAX, Math.max(EDITOR_FONT_SIZE_MIN, record.editorFontSize)))
    : DEFAULT_EDITOR_FONT_SIZE;
  return {
    uiFont: sanitizeFontFamily(record.uiFont),
    editorFont: sanitizeFontFamily(record.editorFont),
    cjkFont: sanitizeFontFamily(record.cjkFont),
    editorFontSize: size
  };
}

/**
 * 按字号计算编辑器行高。
 *
 * @param fontSize 字号像素
 * @returns 行高像素
 */
export function editorLineHeight(fontSize: number): number {
  return Math.round(fontSize * 1.6);
}

/**
 * 把逗号分隔的字族写成带引号的 CSS 字族列表。
 *
 * @param family 用户填写的字族
 * @returns CSS font-family 片段
 */
function quoteFamily(family: string): string {
  return family
    .split(",")
    .map((part) => part.trim().replace(/^["']|["']$/g, ""))
    .filter(Boolean)
    .map((part) => `"${part.replaceAll('"', "")}"`)
    .join(", ");
}

/**
 * 生成 Monaco 可直接使用的字体选项。
 *
 * Monaco 画布读不到 CSS 变量，所以这里展开成具体字族和像素。
 *
 * @param preferences 字体偏好
 * @returns 字族、字号与行高
 */
export function monacoFontOptions(preferences: FontPreferences): { fontFamily: string; fontSize: number; lineHeight: number } {
  const normalized = normalizeFontPreferences(preferences);
  const latin = quoteFamily(normalized.editorFont);
  const cjk = quoteFamily(normalized.cjkFont);
  const fontFamily = [latin, EDITOR_LATIN_FALLBACK, cjk, CJK_MONO_FALLBACK].filter(Boolean).join(", ");
  return {
    fontFamily: `${fontFamily}, monospace`,
    fontSize: normalized.editorFontSize,
    lineHeight: editorLineHeight(normalized.editorFontSize)
  };
}

/**
 * 判断滚轮事件是否要调整编辑器字号。
 *
 * 向上滚动增大，向下滚动减小。只响应 Ctrl，避免和系统缩放手势以外的修饰键混在一起。
 *
 * @param event 滚轮事件的修饰键与滚动量
 * @returns 字号增量；不处理时为 0
 */
export function editorFontWheelDelta(event: { ctrlKey: boolean; metaKey: boolean; deltaY: number }): number {
  if (!event.ctrlKey || event.metaKey || event.deltaY === 0) return 0;
  return event.deltaY < 0 ? 1 : -1;
}

/**
 * 把字体偏好写到文档根节点。
 *
 * @param preferences 字体偏好
 * @returns 无
 */
export function applyFontPreferences(preferences: FontPreferences): void {
  const root = globalThis.document?.documentElement;
  if (!root) return;
  const normalized = normalizeFontPreferences(preferences);
  const uiFamily = quoteFamily(normalized.uiFont);
  const editorFamily = quoteFamily(normalized.editorFont);
  const cjkFamily = quoteFamily(normalized.cjkFont);
  // 1. 界面字族插在内置字族之前，留空则恢复样式表
  if (uiFamily) {
    root.style.setProperty("--font-ui", `${uiFamily}, var(--font-ui-builtin)`);
  } else {
    root.style.removeProperty("--font-ui");
  }
  // 2. 编辑器字族同样保留代码字族作为回退
  if (editorFamily) {
    root.style.setProperty("--font-editor", `${editorFamily}, var(--font-code)`);
  } else {
    root.style.removeProperty("--font-editor");
  }
  // 3. 中文字族插在等宽中文之前；留空时样式表默认就是等宽中文
  if (cjkFamily) {
    root.style.setProperty("--font-cjk", `${cjkFamily}, var(--font-cjk-mono), var(--font-cjk-proportional)`);
  } else {
    root.style.removeProperty("--font-cjk");
  }
  root.style.setProperty("--editor-font-size", `${normalized.editorFontSize}px`);
  root.style.setProperty("--editor-line-height", `${editorLineHeight(normalized.editorFontSize)}px`);
}

/**
 * 读取已保存的字体偏好。
 *
 * @returns 归一化后的偏好
 */
export function loadFontPreferences(): FontPreferences {
  try {
    const raw = globalThis.localStorage?.getItem(FONT_PREFERENCES_STORAGE_KEY);
    return normalizeFontPreferences(raw ? JSON.parse(raw) as unknown : null);
  } catch {
    return { ...DEFAULT_FONT_PREFERENCES };
  }
}

/**
 * 保存并立即应用字体偏好。
 *
 * @param preferences 字体偏好
 * @returns 无
 */
export function saveFontPreferences(preferences: FontPreferences): void {
  const normalized = normalizeFontPreferences(preferences);
  globalThis.localStorage?.setItem(FONT_PREFERENCES_STORAGE_KEY, JSON.stringify(normalized));
  applyFontPreferences(normalized);
  currentSnapshot = normalized;
  listeners.forEach((listener) => listener());
}

/**
 * 按增量调整编辑器字号并保存。
 *
 * @param delta 字号增量，通常为 1 或 -1
 * @returns 调整后的字号
 */
export function adjustEditorFontSize(delta: number): number {
  const current = currentSnapshot ?? loadFontPreferences();
  if (delta === 0) return current.editorFontSize;
  const next = normalizeFontPreferences({ ...current, editorFontSize: current.editorFontSize + delta });
  saveFontPreferences(next);
  return next.editorFontSize;
}

/** 在窗口捕获阶段监听，避免 Monaco 在冒泡前吞掉滚轮。 */
export const EDITOR_FONT_WHEEL_OPTIONS: AddEventListenerOptions = { capture: true, passive: false };

/**
 * 在编辑器容器上监听 Ctrl+滚轮，并在同一轮事件里改字号。
 *
 * @param target 编辑器根节点
 * @param apply 字号写入偏好后的同步回调，供 Monaco 立刻重排
 * @returns 解除监听的函数
 */
export function bindEditorFontWheel(target: EventTarget, apply?: (size: number) => void): () => void {
  const host = target instanceof Node ? target : null;
  const onWheel = (event: Event) => {
    const wheel = event as WheelEvent;
    // 1. 只处理落在编辑区里的滚轮，窗口捕获早于编辑器自己的监听
    if (host && wheel.target instanceof Node && !host.contains(wheel.target)) return;
    const delta = editorFontWheelDelta(wheel);
    if (delta === 0) return;
    // 2. 拦住浏览器和桌面壳的整页缩放，也不让编辑器把这次滚轮当成滚动
    wheel.preventDefault();
    wheel.stopPropagation();
    // 3. 先写入偏好，再把新字号交给当前编辑器实例
    const size = adjustEditorFontSize(delta);
    apply?.(size);
  };
  window.addEventListener("wheel", onWheel, EDITOR_FONT_WHEEL_OPTIONS);
  return () => window.removeEventListener("wheel", onWheel, EDITOR_FONT_WHEEL_OPTIONS);
}

/**
 * 启动时应用已保存的字体偏好。
 *
 * @returns 无
 */
export function initializeFontPreferences(): void {
  const preferences = loadFontPreferences();
  currentSnapshot = preferences;
  applyFontPreferences(preferences);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): FontPreferences {
  if (!currentSnapshot) currentSnapshot = loadFontPreferences();
  return currentSnapshot;
}

/**
 * 订阅当前浏览器的界面字体与编辑器字体。
 *
 * @returns 当前偏好、局部更新与重置
 */
export function useFontPreferences() {
  const preferences = useSyncExternalStore(subscribe, getSnapshot, () => DEFAULT_FONT_PREFERENCES);
  const setPreferences = useCallback((patch: Partial<FontPreferences>) => {
    saveFontPreferences({ ...getSnapshot(), ...patch });
  }, []);
  const resetPreferences = useCallback(() => {
    saveFontPreferences(DEFAULT_FONT_PREFERENCES);
  }, []);
  return { preferences, setPreferences, resetPreferences };
}

/**
 * 让 Monaco 实例跟随编辑器字体，并在编辑区用 Ctrl+滚轮实时改字号。
 *
 * @param hostRef 编辑器容器
 * @param editor 已挂载的 Monaco 实例，未就绪时为 null
 * @returns 当前字体偏好，供编辑器首次渲染使用
 */
export function useMonacoEditorFont(
  hostRef: RefObject<HTMLElement | null>,
  editor: { updateOptions: (options: { fontFamily: string; fontSize: number; lineHeight: number }) => void } | null
): FontPreferences {
  const { preferences } = useFontPreferences();
  useEffect(() => {
    editor?.updateOptions(monacoFontOptions(preferences));
  }, [editor, preferences]);
  useEffect(() => {
    const host = hostRef.current;
    if (!host || !editor) return;
    return bindEditorFontWheel(host, (size) => {
      editor.updateOptions(monacoFontOptions({ ...getSnapshot(), editorFontSize: size }));
    });
  }, [editor, hostRef]);
  return preferences;
}

/**
 * 在编辑器容器上响应 Ctrl+滚轮。
 *
 * @param hostRef 编辑器容器
 * @param enabled 是否接管滚轮；预览模式关闭，避免改到看不见的源码字号
 * @param apply 字号变化后的同步回调
 * @returns 无
 */
export function useEditorFontWheel(
  hostRef: RefObject<HTMLElement | null>,
  enabled = true,
  apply?: (size: number) => void
): void {
  const applyRef = useRef(apply);
  applyRef.current = apply;
  useEffect(() => {
    const host = hostRef.current;
    if (!enabled || !host) return;
    return bindEditorFontWheel(host, (size) => applyRef.current?.(size));
  }, [enabled, hostRef]);
}
