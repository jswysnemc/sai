import { indentLess, indentMore, insertTab } from "@codemirror/commands";
import { EditorSelection, type EditorState, type StateCommand } from "@codemirror/state";
import { analyzeLinePrefix } from "../wysiwyg-line-prefix";
import { analyzeFencedCode } from "../wysiwyg-code-block";
import { fencedCodeAt } from "./block-commands";

/** 行首列表前缀。 */
const LIST_LINE = /^([ \t]*)([-*+]|\d{1,9}[.)])([ \t]+)/;

/**
 * Enter：补全围栏与公式块，以及在标题开头插入空行。
 *
 * 1. 在未闭合的 ``` 或 $$ 行尾回车：自动补上闭栏，光标落在块内第一行
 * 2. 在标题正文开头回车：在标题上方插入空行，标题保持不动（对齐 Typora）
 * 3. 其余情况返回 false，交给 Markdown 自带的列表续写与普通换行
 *
 * @returns CodeMirror 状态命令
 */
export function smartEnter(): StateCommand {
  return ({ state, dispatch }) => {
    const range = state.selection.main;
    if (!range.empty || state.selection.ranges.length > 1) return false;
    const line = state.doc.lineAt(range.head);
    // 1. 未闭合的围栏或公式开栏
    const fence = /^([ \t]*)(`{3,}|~{3,})([\w+#.-]*)[ \t]*$/.exec(line.text);
    if (fence && range.head === line.to) {
      const block = fencedCodeAt(state, line.from);
      const info = block && block.from === line.from ? analyzeFencedCode(state, block) : null;
      // 已闭合的空代码块照常换行；未闭合、或刚输入的开栏与下方某个围栏误配对时补全闭栏
      const emptyClosed = info?.closeLine && info.contentLines.every((content) => !content.text.trim());
      if (info && !emptyClosed) {
        const insert = `\n${fence[1]}\n${fence[1]}${fence[2]}`;
        dispatch(state.update({ changes: { from: line.to, insert }, selection: { anchor: line.to + fence[1].length + 1 }, userEvent: "input" }));
        return true;
      }
    }
    if (/^[ \t]*\$\$[ \t]*$/.test(line.text) && range.head === line.to && !closingMathBelow(state, line.number)) {
      dispatch(state.update({ changes: { from: line.to, insert: "\n\n$$" }, selection: { anchor: line.to + 1 }, userEvent: "input" }));
      return true;
    }
    // 2. 标题正文开头
    const prefix = analyzeLinePrefix(state, line);
    if (prefix?.marker?.kind === "heading" && range.head === prefix.to && prefix.to < line.to) {
      dispatch(state.update({ changes: { from: line.from, insert: "\n" }, selection: { anchor: range.head + 1 }, userEvent: "input" }));
      return true;
    }
    return false;
  };
}

/**
 * 判断 $$ 行下方是否已有闭栏，避免在闭栏行回车时重复补全。
 *
 * @param state 编辑器状态
 * @param lineNumber 当前行号
 * @returns 当前行是某个公式块的闭栏时为 true
 */
function closingMathBelow(state: EditorState, lineNumber: number): boolean {
  // 向上数 $$ 行的个数：奇数说明当前行是闭栏
  let count = 0;
  for (let number = 1; number < lineNumber; number += 1) {
    if (/^[ \t]*\$\$[ \t]*$/.test(state.doc.line(number).text)) count += 1;
  }
  return count % 2 === 1;
}

/**
 * Backspace：在标题开头降为正文；空代码块整块删除；代码块开头不合并围栏。
 *
 * 列表与引用的回退交给 Markdown 自带的 deleteMarkupBackward。
 *
 * @returns CodeMirror 状态命令
 */
export function smartBackspace(): StateCommand {
  return ({ state, dispatch }) => {
    const range = state.selection.main;
    if (!range.empty || state.selection.ranges.length > 1) return false;
    const line = state.doc.lineAt(range.head);
    // 1. 标题正文开头：去掉井号降为正文
    const prefix = analyzeLinePrefix(state, line);
    if (prefix?.marker?.kind === "heading" && range.head === prefix.to && prefix.to > line.from) {
      const mark = /^[ \t]{0,3}#{1,6}[ \t]+/.exec(state.doc.sliceString(line.from, line.to).replace(/^(?:[ \t]{0,3}>[ \t]?)*/, ""));
      const from = prefix.to - (mark?.[0].length ?? 0);
      dispatch(state.update({ changes: { from, to: prefix.to }, selection: { anchor: from }, userEvent: "delete" }));
      return true;
    }
    // 2. 代码块首个内容行开头
    const node = fencedCodeAt(state);
    if (!node) return false;
    const block = analyzeFencedCode(state, node);
    const first = block.contentLines[0];
    if (!first || range.head !== first.from || !block.closeLine) return false;
    const empty = block.contentLines.every((content) => !content.text.trim());
    if (!empty) return true;
    // 空代码块：整块删除，留下一个空行
    dispatch(
      state.update({
        changes: { from: block.openLine.from, to: block.closeLine.to },
        selection: { anchor: block.openLine.from },
        userEvent: "delete",
      })
    );
    return true;
  };
}

/**
 * Tab / Shift-Tab：列表项缩进与反缩进，代码块内缩进，其余位置插入制表符。
 *
 * 列表缩进以上一个同级项的正文起点为准，有序列表的子项因此能正确对齐。
 *
 * @param outdent 是否为反缩进
 * @returns CodeMirror 状态命令
 */
export function smartTab(outdent: boolean): StateCommand {
  return (target) => {
    const { state, dispatch } = target;
    const line = state.doc.lineAt(state.selection.main.head);
    const list = LIST_LINE.exec(line.text);
    if (list && state.selection.ranges.length === 1) {
      const indent = list[1].length;
      const next = outdent ? parentIndent(state, line.number, indent) : siblingContentIndent(state, line.number, indent);
      if (next === null || next === indent) return true;
      const delta = next - indent;
      dispatch(
        state.update({
          changes: { from: line.from, to: line.from + indent, insert: " ".repeat(next) },
          selection: EditorSelection.cursor(Math.max(line.from + next, state.selection.main.head + delta)),
          userEvent: "input.indent",
        })
      );
      return true;
    }
    if (fencedCodeAt(state)) return outdent ? indentLess(target) : state.selection.main.empty ? insertTab(target) : indentMore(target);
    return outdent ? indentLess(target) : false;
  };
}

/**
 * 找到上方最近的同级列表项，返回其正文起点列，作为缩进后的列。
 *
 * @param state 编辑器状态
 * @param lineNumber 当前行号
 * @param indent 当前缩进
 * @returns 缩进后的列；上方没有同级项（是首项）时为 null
 */
function siblingContentIndent(state: EditorState, lineNumber: number, indent: number): number | null {
  for (let number = lineNumber - 1; number >= 1; number -= 1) {
    const text = state.doc.line(number).text;
    if (!text.trim()) continue;
    const match = LIST_LINE.exec(text);
    if (!match) {
      if (/^\S/.test(text)) return null;
      continue;
    }
    if (match[1].length === indent) return match[0].length;
    if (match[1].length < indent) return null;
  }
  return null;
}

/**
 * 找到上方最近的父级列表项，返回其缩进列，作为反缩进后的列。
 *
 * @param state 编辑器状态
 * @param lineNumber 当前行号
 * @param indent 当前缩进
 * @returns 反缩进后的列；已在顶层时为 null
 */
function parentIndent(state: EditorState, lineNumber: number, indent: number): number | null {
  if (indent === 0) return null;
  for (let number = lineNumber - 1; number >= 1; number -= 1) {
    const match = LIST_LINE.exec(state.doc.line(number).text);
    if (match && match[1].length < indent) return match[1].length;
  }
  return 0;
}
