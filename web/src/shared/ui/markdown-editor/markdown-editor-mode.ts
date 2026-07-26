/**
 * Markdown 编辑器的三种显示模式。
 *
 * source   纯源码，保留全部语法标记，适合精确修改
 * wysiwyg  所见即所得，非光标行隐藏语法标记并直接呈现排版
 * preview  只读渲染结果
 */
export type MarkdownEditorMode = "source" | "wysiwyg" | "preview";

/** 模式循环顺序，供快捷键切换使用。 */
export const MARKDOWN_EDITOR_MODES: MarkdownEditorMode[] = ["source", "wysiwyg", "preview"];

/**
 * 判断该模式下文本是否可编辑。
 *
 * @param mode 当前模式
 * @returns 可编辑时为 true
 */
export function isEditableMode(mode: MarkdownEditorMode): boolean {
  return mode !== "preview";
}

/**
 * 取循环切换的下一个模式。
 *
 * @param mode 当前模式
 * @returns 顺序中的下一个模式，末尾回到开头
 */
export function nextMarkdownMode(mode: MarkdownEditorMode): MarkdownEditorMode {
  const index = MARKDOWN_EDITOR_MODES.indexOf(mode);
  return MARKDOWN_EDITOR_MODES[(index + 1) % MARKDOWN_EDITOR_MODES.length];
}
