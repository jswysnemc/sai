/**
 * Markdown 编辑器的两种显示模式。
 *
 * source   纯源码，保留全部语法标记，适合精确修改
 * preview  所见即所得预览：直接呈现排版并可就地编辑，行为对齐 Typora
 */
export type MarkdownEditorMode = "source" | "preview";

/** 模式循环顺序，供快捷键切换使用。 */
export const MARKDOWN_EDITOR_MODES: MarkdownEditorMode[] = ["preview", "source"];

/** 缺省模式：打开文档即呈现排版。 */
export const DEFAULT_MARKDOWN_EDITOR_MODE: MarkdownEditorMode = "preview";

/**
 * 把任意输入归一为合法模式。
 *
 * 旧版本的三态里还有 "wysiwyg"，它与现在的预览同义，按预览处理。
 *
 * @param value 任意来源的模式值，如本地存储
 * @returns 合法模式，无法识别时为缺省模式
 */
export function normalizeMarkdownMode(value: unknown): MarkdownEditorMode {
  return value === "source" ? "source" : DEFAULT_MARKDOWN_EDITOR_MODE;
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
