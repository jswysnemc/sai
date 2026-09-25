import { useState } from "react";
import { normalizeMarkdownMode, type MarkdownEditorMode } from "./markdown-editor-mode";

/** 模式偏好的存储键。 */
const STORAGE_KEY = "sai.markdown-editor.mode";

/**
 * 读取上次使用的 Markdown 显示模式。
 *
 * @returns 已归一的模式；存储不可用时为缺省模式
 */
function readMode(): MarkdownEditorMode {
  try {
    return normalizeMarkdownMode(globalThis.localStorage?.getItem(STORAGE_KEY));
  } catch {
    return normalizeMarkdownMode(null);
  }
}

/**
 * 读写 Markdown 显示模式，并记住用户的选择。
 *
 * @returns 当前模式与设置函数
 */
export function useMarkdownMode(): [MarkdownEditorMode, (mode: MarkdownEditorMode) => void] {
  const [mode, setMode] = useState(readMode);
  const update = (next: MarkdownEditorMode) => {
    setMode(next);
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, next);
    } catch {
      // 存储不可用时只在本次会话内生效
    }
  };
  return [mode, update];
}
