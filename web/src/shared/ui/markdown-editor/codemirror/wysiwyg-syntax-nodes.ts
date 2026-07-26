/**
 * Lezer Markdown 语法树的节点名分类。
 *
 * 所见即所得模式靠这些分类决定：哪些节点是纯语法标记（离开该行就隐藏），
 * 哪些节点承载内容（始终附加排版样式）。节点名取自 @lezer/markdown 的
 * 基础语法与 GFM 扩展，改动时需与该包的定义保持一致。
 */

/**
 * 纯语法标记节点：光标不在所在行时整体隐藏。
 *
 * 例如标题的 `#`、加粗的 `**`、引用的 `>`。
 */
export const SYNTAX_MARK_NODES = new Set([
  "HeaderMark",
  "EmphasisMark",
  "CodeMark",
  "LinkMark",
  "QuoteMark",
  "StrikethroughMark",
  "CommentBlock",
]);

/** 内容节点到排版样式类的映射，始终生效。 */
export const CONTENT_STYLE_CLASSES: Record<string, string> = {
  ATXHeading1: "cm-md-heading cm-md-h1",
  ATXHeading2: "cm-md-heading cm-md-h2",
  ATXHeading3: "cm-md-heading cm-md-h3",
  ATXHeading4: "cm-md-heading cm-md-h4",
  ATXHeading5: "cm-md-heading cm-md-h5",
  ATXHeading6: "cm-md-heading cm-md-h6",
  SetextHeading1: "cm-md-heading cm-md-h1",
  SetextHeading2: "cm-md-heading cm-md-h2",
  Emphasis: "cm-md-emphasis",
  StrongEmphasis: "cm-md-strong",
  Strikethrough: "cm-md-strikethrough",
  InlineCode: "cm-md-inline-code",
  FencedCode: "cm-md-fenced-code",
  CodeText: "cm-md-code-text",
  Blockquote: "cm-md-blockquote",
  URL: "cm-md-url",
  Link: "cm-md-link",
  TableHeader: "cm-md-table-header",
  TableDelimiter: "cm-md-table-delimiter",
  CodeInfo: "cm-md-code-info",
};

/**
 * 判断节点是否为纯语法标记。
 *
 * @param name 语法树节点名
 * @returns 是标记节点时为 true
 */
export function isSyntaxMark(name: string): boolean {
  return SYNTAX_MARK_NODES.has(name);
}

/**
 * 取节点对应的排版样式类。
 *
 * @param name 语法树节点名
 * @returns 样式类名；无对应样式时为 undefined
 */
export function styleClassFor(name: string): string | undefined {
  return CONTENT_STYLE_CLASSES[name];
}
