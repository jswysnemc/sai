import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import { Bold, Code, Heading1, Heading2, Heading3, Italic, Link, Pilcrow, Quote, Strikethrough, type LucideIcon } from "lucide-react";
import { modKeyLabel } from "../../../mod-key";
import { setHeading, toggleQuote } from "./commands/block-commands";
import { BOLD, INLINE_CODE, insertLink, ITALIC, STRIKE, toggleInline, type InlineFormat } from "./commands/inline-commands";
import { analyzeLinePrefix } from "./wysiwyg-line-prefix";

/** 双语文案取值函数。 */
export type Translate = (en: string, zh: string) => string;

/** 一个格式动作：图标按钮与菜单共用。 */
export type FormatAction = {
  id: string;
  icon: LucideIcon;
  label: string;
  shortcut?: string;
  run: (view: EditorView) => boolean;
  /** 当前光标或选区是否已处于该格式 */
  active: (state: EditorState) => boolean;
};

/**
 * 判断选区起点是否位于指定行内格式中。
 *
 * @param state 编辑器状态
 * @param format 行内格式
 * @returns 位于其中时为 true
 */
function inFormat(state: EditorState, format: InlineFormat | { node: string }): boolean {
  const { from } = state.selection.main;
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(from, 1); node; node = node.parent) {
    if (node.name === format.node) return true;
  }
  return false;
}

/**
 * 读取光标所在行的标题级别。
 *
 * @param state 编辑器状态
 * @returns 标题级别；正文为 0
 */
export function headingLevelAt(state: EditorState): number {
  const prefix = analyzeLinePrefix(state, state.doc.lineAt(state.selection.main.head));
  return prefix?.marker?.kind === "heading" ? prefix.marker.level : 0;
}

/**
 * 构建块级格式动作：正文与各级标题。
 *
 * @param t 双语文案取值函数
 * @returns 动作列表
 */
export function blockFormatActions(t: Translate): FormatAction[] {
  const mod = modKeyLabel();
  const heading = (level: number, icon: LucideIcon, label: string): FormatAction => ({
    id: `h${level}`,
    icon,
    label,
    shortcut: `${mod}+${level}`,
    run: (view) => setHeading(level)(view),
    active: (state) => headingLevelAt(state) === level,
  });
  return [
    heading(0, Pilcrow, t("Paragraph", "正文")),
    heading(1, Heading1, t("Heading 1", "标题 1")),
    heading(2, Heading2, t("Heading 2", "标题 2")),
    heading(3, Heading3, t("Heading 3", "标题 3")),
  ];
}

/**
 * 构建行内格式动作：加粗、斜体、删除线、行内代码与链接。
 *
 * @param t 双语文案取值函数
 * @returns 动作列表
 */
export function inlineFormatActions(t: Translate): FormatAction[] {
  const mod = modKeyLabel();
  const inline = (id: string, icon: LucideIcon, label: string, format: InlineFormat, shortcut: string): FormatAction => ({
    id,
    icon,
    label,
    shortcut,
    run: (view) => toggleInline(format)(view),
    active: (state) => inFormat(state, format),
  });
  return [
    inline("bold", Bold, t("Bold", "加粗"), BOLD, `${mod}+B`),
    inline("italic", Italic, t("Italic", "斜体"), ITALIC, `${mod}+I`),
    inline("strike", Strikethrough, t("Strikethrough", "删除线"), STRIKE, `${mod}+Shift+X`),
    inline("code", Code, t("Inline code", "行内代码"), INLINE_CODE, `${mod}+E`),
    {
      id: "link",
      icon: Link,
      label: t("Link", "链接"),
      shortcut: `${mod}+K`,
      run: (view) => insertLink()(view),
      active: (state) => inFormat(state, { node: "Link" }),
    },
  ];
}

/**
 * 构建引用动作。
 *
 * @param t 双语文案取值函数
 * @returns 引用动作
 */
export function quoteAction(t: Translate): FormatAction {
  return {
    id: "quote",
    icon: Quote,
    label: t("Quote", "引用"),
    shortcut: `${modKeyLabel()}+Shift+Q`,
    run: (view) => toggleQuote()(view),
    active: (state) => inFormat(state, { node: "Blockquote" }),
  };
}
