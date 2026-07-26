import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

/**
 * 两种模式共用的基础主题。
 *
 * 颜色一律引用设计令牌，主题切换时无需重建编辑器实例。
 */
const baseTheme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--ink)",
    backgroundColor: "transparent",
  },
  ".cm-scroller": {
    lineHeight: "1.7",
    padding: "var(--space-md) 0",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--ink)",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
    backgroundColor: "var(--signal-soft)",
  },
  ".cm-gutters": {
    border: "none",
    backgroundColor: "transparent",
    color: "var(--ink-soft)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "transparent",
    color: "var(--ink)",
  },
});

/** 所见即所得模式：正文字体、居中版心，读起来接近成稿。 */
const liveTheme = EditorView.theme({
  "&": { fontSize: "var(--text-base)" },
  ".cm-scroller": { fontFamily: "var(--font-ui)" },
  ".cm-content": {
    maxWidth: "48rem",
    margin: "0 auto",
    padding: "0 var(--space-lg)",
  },
});

/** 源码模式：等宽字体，保留完整语法标记。 */
const sourceTheme = EditorView.theme({
  "&": { fontSize: "var(--text-sm)" },
  ".cm-scroller": { fontFamily: "var(--font-code)" },
  ".cm-content": { padding: "0 var(--space-sm)" },
});

/** 围栏代码块内部的语法着色，沿用代码视图的同一组令牌。 */
const codeHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: "var(--code-keyword)" },
  { tag: [tags.string, tags.special(tags.string), tags.regexp], color: "var(--code-string)" },
  { tag: [tags.number, tags.bool, tags.atom, tags.null], color: "var(--code-number)" },
  { tag: tags.comment, color: "var(--code-comment)", fontStyle: "italic" },
  { tag: [tags.typeName, tags.className, tags.attributeName], color: "var(--code-number)" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: "var(--blue)" },
  { tag: [tags.propertyName, tags.tagName], color: "var(--code-keyword)" },
  { tag: tags.invalid, color: "var(--code-danger)" },
]);

/**
 * 组装主题扩展。
 *
 * @param dark 是否为深色主题，供 CodeMirror 调整内建控件配色
 * @param live 是否为所见即所得模式
 * @returns 主题相关扩展
 */
export function editorTheme(dark: boolean, live: boolean): Extension[] {
  return [
    baseTheme,
    live ? liveTheme : sourceTheme,
    EditorView.theme({}, { dark }),
    syntaxHighlighting(codeHighlight),
  ];
}
