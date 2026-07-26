import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { StateField, type EditorState, type Line, type Range } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";
import type { SyntaxNodeRef } from "@lezer/common";
import { analyzeFencedCode } from "./wysiwyg-code-block";
import { buildTableModel } from "./wysiwyg-table-model";
import { TableWidget } from "./wysiwyg-table-widget";

/** 整体解析预算（毫秒），超时后退回已有的部分语法树。 */
const PARSE_BUDGET_MS = 80;

/**
 * 块级所见即所得装饰。
 *
 * 表格替换为部件、围栏行整行隐藏都会改变纵向布局，
 * CodeMirror 要求这类装饰由 StateField 提供，不能放进 ViewPlugin，
 * 因此与行内装饰（wysiwyg-decorations）分成两层。
 */
export const wysiwygBlockDecorations = StateField.define<DecorationSet>({
  create: buildBlockDecorations,
  update(value, transaction) {
    if (transaction.docChanged || transaction.selection) {
      return buildBlockDecorations(transaction.state);
    }
    return value.map(transaction.changes);
  },
  provide: (field) => EditorView.decorations.from(field),
});

/**
 * 扫描全文档并收集块级装饰。
 *
 * 参数:
 * - `state`: 编辑器状态
 *
 * 返回:
 * - 排序后的装饰集合
 */
export function buildBlockDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  // 块级装饰不随视口重建，必须把整个文档解析完，否则视口外的表格不会渲染
  const tree = ensureSyntaxTree(state, state.doc.length, PARSE_BUDGET_MS) ?? syntaxTree(state);
  tree.iterate({
    enter: (node) => {
      if (node.name === "Table") return collectTable(state, node, ranges);
      if (node.name === "FencedCode") return collectFencedCode(state, node, ranges);
      return true;
    },
  });
  return Decoration.set(ranges, true);
}

/**
 * 判断选区是否触及给定范围。
 *
 * @param state 编辑器状态
 * @param from 范围起点
 * @param to 范围终点
 * @returns 任一选区与范围相交时为 true
 */
function selectionTouches(state: EditorState, from: number, to: number): boolean {
  return state.selection.ranges.some((range) => range.from <= to && range.to >= from);
}

/**
 * 处理表格节点。
 *
 * @param state 编辑器状态
 * @param node Table 语法节点
 * @param ranges 装饰收集容器
 * @returns 是否继续遍历子节点
 */
function collectTable(
  state: EditorState,
  node: SyntaxNodeRef,
  ranges: Range<Decoration>[]
): boolean {
  // 1. 光标在表格内时不替换，交给行内装饰呈现源码编辑态
  if (selectionTouches(state, node.from, node.to)) return true;
  // 2. 去掉节点末尾可能带上的空白，块级替换必须精确落在行边界
  let end = node.to;
  while (end > node.from && /\s/.test(state.doc.sliceString(end - 1, end))) end -= 1;
  const startLine = state.doc.lineAt(node.from);
  const endLine = state.doc.lineAt(end);
  // 嵌套在引用等容器里的表格不独占整行，退回源码展示
  if (node.from !== startLine.from || end !== endLine.to) return true;
  const model = buildTableModel(state, node.node);
  if (!model) return true;
  const source = state.doc.sliceString(node.from, end);
  const widget = new TableWidget(model, source, node.from);
  ranges.push(Decoration.replace({ widget, block: true }).range(node.from, end));
  return false;
}

/**
 * 处理围栏代码块节点。
 *
 * @param state 编辑器状态
 * @param node FencedCode 语法节点
 * @param ranges 装饰收集容器
 * @returns 恒为 false，代码块内部不再深入
 */
function collectFencedCode(
  state: EditorState,
  node: SyntaxNodeRef,
  ranges: Range<Decoration>[]
): boolean {
  const block = analyzeFencedCode(state, node);
  const inside = selectionTouches(state, node.from, node.to);
  // 空代码块的围栏不隐藏，全部隐藏后用户将无法再定位到它
  const hideFences = !inside && block.contentLines.length > 0;
  if (hideFences) {
    appendHiddenLine(block.openLine, ranges);
    if (block.closeLine) appendHiddenLine(block.closeLine, ranges);
  }
  // 无论光标内外，代码块整体保持块级底色，编辑时视觉不跳变
  const visible: Line[] = hideFences
    ? block.contentLines
    : [block.openLine, ...block.contentLines, ...(block.closeLine ? [block.closeLine] : [])];
  visible.forEach((line, index) => {
    const classes = ["cm-md-codeline"];
    if (index === 0) classes.push("cm-md-codeline-first");
    if (index === visible.length - 1) classes.push("cm-md-codeline-last");
    // 围栏隐藏后语言标识随之不可见，转为角标挂在首行上
    const attributes =
      index === 0 && hideFences && block.language ? { "data-md-lang": block.language } : undefined;
    ranges.push(Decoration.line({ class: classes.join(" "), attributes }).range(line.from));
  });
  return false;
}

/**
 * 隐藏一整行（围栏行）。
 *
 * @param line 待隐藏的行
 * @param ranges 装饰收集容器
 * @returns 无
 */
function appendHiddenLine(line: Line, ranges: Range<Decoration>[]): void {
  ranges.push(Decoration.replace({ block: true }).range(line.from, line.to));
}
