import { syntaxTree } from "@codemirror/language";
import type { EditorState, Line } from "@codemirror/state";
import type { SyntaxNode, Tree } from "@lezer/common";

/** 行首元素：决定前缀被替换成什么。 */
export type LineMarker =
  | { kind: "bullet" }
  | { kind: "ordered"; label: string }
  | { kind: "task"; checked: boolean; markerFrom: number; markerTo: number }
  | { kind: "heading"; level: number };

/**
 * 一行的隐藏前缀。
 *
 * 所见即所得模式下，行首的 `>`、`#`、列表符号与缩进统一视为"前缀"：
 * 从行首到 to 的原文整体隐藏，由行样式（缩进、引用竖线）和部件（圆点、勾选框）代为呈现。
 */
export type LinePrefix = {
  /** 行首偏移 */
  from: number;
  /** 前缀结束偏移，此处起为可见正文 */
  to: number;
  /** 引用层数 */
  quoteDepth: number;
  /** 列表嵌套层数，0 表示不在列表里 */
  listDepth: number;
  /** 行首元素；续行等只有缩进的前缀为 null */
  marker: LineMarker | null;
};

/** 这些块内部的行保持原样，由各自的块级装饰负责呈现。 */
const OPAQUE_BLOCKS = new Set(["FencedCode", "CodeBlock", "BlockMath", "HTMLBlock", "CommentBlock", "Table"]);

/** 标题前缀：至多三个空格缩进、一到六个井号、至少一个空白。 */
const HEADING_PREFIX = /^([ \t]{0,3})(#{1,6})[ \t]+/;

/** 列表前缀：缩进、符号或序号、至少一个空白，可选任务框。 */
const LIST_PREFIX = /^([ \t]*)([-*+]|\d{1,9}[.)])[ \t]+(?:(\[[ xX]\])(?:[ \t]+|$))?/;

/** 引用前缀：一个或多个 `>`，各自可带缩进与一个空白。 */
const QUOTE_PREFIX = /^(?:[ \t]{0,3}>[ \t]?)+/;

/**
 * 分析一行的隐藏前缀。
 *
 * 先用正则切出候选前缀，再用 Lezer 语法树确认其语义，
 * 避免把代码块里的 `#`、段落里的连字符误当成标记。
 * 只有 `#` 或 `-` 而没有后随空白时（正在输入）不视为前缀，避免输入过程中闪烁。
 *
 * @param state 编辑器状态
 * @param line 待分析的行
 * @param tree 语法树，缺省取当前状态的树
 * @returns 行前缀；该行不需要特殊处理时为 null
 */
export function analyzeLinePrefix(state: EditorState, line: Line, tree: Tree = syntaxTree(state)): LinePrefix | null {
  const text = line.text;
  if (!text.trim()) return null;
  // 1. 引用标记：语法树确认首个 > 确为 QuoteMark
  const quoteMatch = QUOTE_PREFIX.exec(text);
  const firstQuote = quoteMatch ? text.indexOf(">") : -1;
  const quoteLength = quoteMatch && nodeAt(tree, line.from + firstQuote)?.name === "QuoteMark" ? quoteMatch[0].length : 0;
  const contentStart = line.from + quoteLength;
  // 2. 代码块、表格等不透明块内部不处理
  const context = tree.resolveInner(contentStart, 1);
  if (hasAncestor(context, OPAQUE_BLOCKS)) return null;
  const quoteDepth = countAncestors(context, "Blockquote");
  const rest = text.slice(quoteLength);
  // 3. 标题
  const heading = HEADING_PREFIX.exec(rest);
  if (heading && nodeAt(tree, contentStart + heading[1].length)?.name === "HeaderMark") {
    return {
      from: line.from,
      to: contentStart + heading[0].length,
      quoteDepth,
      listDepth: countLists(context),
      marker: { kind: "heading", level: heading[2].length },
    };
  }
  // 4. 列表项
  const list = LIST_PREFIX.exec(rest);
  const markFrom = list ? contentStart + list[1].length : -1;
  const markNode = list ? nodeAt(tree, markFrom) : null;
  if (list && markNode?.name === "ListMark") {
    return {
      from: line.from,
      to: listPrefixEnd(tree, contentStart, list),
      quoteDepth,
      listDepth: countLists(markNode),
      marker: listMarker(tree, contentStart, list),
    };
  }
  // 5. 列表里的续行：只隐藏缩进，由行样式补回对齐
  const listDepth = countLists(tree.resolveInner(contentStart + (rest.length - rest.trimStart().length), 1));
  if (listDepth === 0 && quoteDepth === 0) return null;
  const indent = listDepth > 0 ? rest.length - rest.trimStart().length : 0;
  return { from: line.from, to: contentStart + indent, quoteDepth, listDepth, marker: null };
}

/**
 * 计算列表前缀的结束位置。
 *
 * 任务框只有被语法树确认为 TaskMarker 时才并入前缀。
 *
 * @param tree 语法树
 * @param contentStart 引用标记之后的偏移
 * @param list 列表正则的命中结果
 * @returns 前缀结束偏移
 */
function listPrefixEnd(tree: Tree, contentStart: number, list: RegExpExecArray): number {
  if (list[3] && isTaskMarker(tree, contentStart, list)) return contentStart + list[0].length;
  const markEnd = list[1].length + list[2].length;
  const spaces = /^[ \t]+/.exec(list[0].slice(markEnd))?.[0].length ?? 0;
  return contentStart + markEnd + spaces;
}

/**
 * 判断列表项的方括号是否为任务框。
 *
 * @param tree 语法树
 * @param contentStart 引用标记之后的偏移
 * @param list 列表正则的命中结果
 * @returns 是 TaskMarker 时为 true
 */
function isTaskMarker(tree: Tree, contentStart: number, list: RegExpExecArray): boolean {
  const offset = list[0].indexOf("[", list[1].length + list[2].length);
  return offset >= 0 && nodeAt(tree, contentStart + offset)?.name === "TaskMarker";
}

/**
 * 构造列表项的行首元素。
 *
 * @param tree 语法树
 * @param contentStart 引用标记之后的偏移
 * @param list 列表正则的命中结果
 * @returns 圆点、序号或任务框
 */
function listMarker(tree: Tree, contentStart: number, list: RegExpExecArray): LineMarker {
  if (list[3] && isTaskMarker(tree, contentStart, list)) {
    const markerFrom = contentStart + list[0].indexOf("[", list[1].length + list[2].length);
    return { kind: "task", checked: /x/i.test(list[3]), markerFrom, markerTo: markerFrom + 3 };
  }
  return /\d/.test(list[2]) ? { kind: "ordered", label: list[2] } : { kind: "bullet" };
}

/**
 * 取紧贴某位置右侧的最内层节点。
 *
 * @param tree 语法树
 * @param position 文档偏移
 * @returns 节点；越界时为 null
 */
function nodeAt(tree: Tree, position: number): SyntaxNode | null {
  return tree.resolveInner(position, 1);
}

/**
 * 判断节点或其祖先是否属于给定集合。
 *
 * @param node 起始节点
 * @param names 节点名集合
 * @returns 命中时为 true
 */
function hasAncestor(node: SyntaxNode, names: ReadonlySet<string>): boolean {
  for (let cursor: SyntaxNode | null = node; cursor; cursor = cursor.parent) {
    if (names.has(cursor.name)) return true;
  }
  return false;
}

/**
 * 统计节点及其祖先中给定名称的数量。
 *
 * @param node 起始节点
 * @param name 节点名
 * @returns 命中数量
 */
function countAncestors(node: SyntaxNode, name: string): number {
  let count = 0;
  for (let cursor: SyntaxNode | null = node; cursor; cursor = cursor.parent) {
    if (cursor.name === name) count += 1;
  }
  return count;
}

/**
 * 统计节点所处的列表嵌套层数。
 *
 * @param node 起始节点
 * @returns 有序与无序列表的总层数
 */
function countLists(node: SyntaxNode): number {
  return countAncestors(node, "BulletList") + countAncestors(node, "OrderedList");
}
