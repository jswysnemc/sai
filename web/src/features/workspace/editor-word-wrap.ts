const STORAGE_KEY = "sai.editor.word-wrap";

/**
 * 读取自动换行偏好。
 *
 * 没有记录时使用调用方给出的默认值：代码文件默认不换行，Markdown 默认换行。
 *
 * @param fallback 本地没有明确记录时返回的值
 * @returns 是否自动换行
 */
export function readEditorWordWrap(fallback = false): boolean {
  try {
    const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (raw === "on") return true;
    if (raw === "off") return false;
    return fallback;
  } catch {
    return fallback;
  }
}

/**
 * 记住自动换行偏好，供代码编辑器和 Markdown 源码共用。
 *
 * @param wrap 是否自动换行
 * @returns 无
 */
export function writeEditorWordWrap(wrap: boolean): void {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, wrap ? "on" : "off");
  } catch {
    // 存储不可用时只保留当前会话里的状态
  }
}
