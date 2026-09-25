import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { StateField, type EditorState, type Line, type Range, type Transaction } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";
import type { SyntaxNodeRef } from "@lezer/common";
import { editorDarkTheme } from "./editor-facets";
import { analyzeFencedCode } from "./wysiwyg-code-block";
import { BlockMathWidget } from "./wysiwyg-math-widgets";
import { MermaidWidget } from "./wysiwyg-mermaid-widget";
import { buildTableModel } from "./wysiwyg-table-model";
import { TableWidget } from "./wysiwyg-table-widget";

/** 整体解析预算（毫秒），超时后退回已有的部分语法树。 */
const PARSE_BUDGET_MS = 60;

/** 光标触及时切换为源码编辑的块：公式与 Mermaid 图表。 */
type RevealableBlock = { from: number; to: number };

/**
 * 受保护的隐藏区域：隐藏的围栏行与表格。
 *
 * 光标不应停在其中（看不见），键盘删除也不应破坏其结构，
 * 由 wysiwyg-guard 据此做光标归位与删除保护。
 */
export type GuardRange = {
  kind: "fence-open" | "fence-close" | "table";
  from: number;
  to: number;
  /** 向后（向左、向上）移动时的归位点 */
  before: number;
  /** 向前（向右、向下）移动时的归位点 */
  after: number;
};

/** 块级装饰状态。 */
export type BlockDecorationState = {
  decorations: DecorationSet;
  /** 受保护的隐藏区域 */
  guards: GuardRange[];
  /** 可显露块的范围，选区变化时据此判断是否需要重建 */
  revealable: RevealableBlock[];
  /** 当前处于显露状态的块，按起点记录 */
  revealedKey: string;
};

/**
 * 块级所见即所得装饰。
 *
 * 表格替换为部件、围栏行整行隐藏都会改变纵向布局，
 * CodeMirror 要求这类装饰由 StateField 提供，不能放进 ViewPlugin，
 * 因此与行内装饰（wysiwyg-decorations）分成两层。
 */
export const wysiwygBlockDecorations = StateField.define<BlockDecorationState>({
  create: (state) => buildBlockState(state),
  update(value, transaction) {
    if (transaction.docChanged || themeChanged(transaction) || treeChanged(transaction)) {
      return buildBlockState(transaction.state);
    }
    // 只移动光标时，仅在进出公式或图表时重建，其余情况原样复用
    if (transaction.selection && revealKey(transaction.state, value.revealable) !== value.revealedKey) {
      return buildBlockState(transaction.state);
    }
    return value;
  },
  provide: (field) => EditorView.decorations.from(field, (value) => value.decorations),
});

/**
 * 兼容旧接口：只取装饰集合。
 *
 * @param state 编辑器状态
 * @returns 排序后的装饰集合
 */
export function buildBlockDecorations(state: EditorState): DecorationSet {
  return buildBlockState(state).decorations;
}

/**
 * 扫描全文档并收集块级装饰。
 *
 * @param state 编辑器状态
 * @returns 块级装饰状态
 */
export function buildBlockState(state: EditorState): BlockDecorationState {
  const ranges: Range<Decoration>[] = [];
  const guards: GuardRange[] = [];
  const revealable: RevealableBlock[] = [];
  // 块级装饰不随视口重建，必须把整个文档解析完，否则视口外的表格不会渲染
  const tree = ensureSyntaxTree(state, state.doc.length, PARSE_BUDGET_MS) ?? syntaxTree(state);
  tree.iterate({
    enter: (node) => {
      if (node.name === "Table") return collectTable(state, node, ranges, guards);
      if (node.name === "FencedCode") return collectFencedCode(state, node, ranges, guards, revealable);
      if (node.name === "BlockMath") return collectBlockMath(state, node, ranges, revealable);
      return true;
    },
  });
  return {
    decorations: Decoration.set(ranges, true),
    guards,
    revealable,
    revealedKey: revealKey(state, revealable),
  };
}

/**
 * 判断选区是否触及给定范围。
 *
 * @param state 编辑器状态
 * @param from 范围起点
 * @param to 范围终点
 * @returns 任一选区与范围相交或相接时为 true
 */
function selectionTouches(state: EditorState, from: number, to: number): boolean {
  return state.selection.ranges.some((range) => range.from <= to && range.to >= from);
}

/**
 * 计算当前处于显露状态的块集合的键。
 *
 * @param state 编辑器状态
 * @param blocks 可显露块
 * @returns 显露块起点拼成的键
 */
function revealKey(state: EditorState, blocks: readonly RevealableBlock[]): string {
  return blocks
    .filter((block) => selectionTouches(state, block.from, block.to))
    .map((block) => block.from)
    .join(",");
}

/**
 * 判断事务是否切换了深浅主题。
 *
 * @param transaction 事务
 * @returns 主题变化时为 true
 */
function themeChanged(transaction: Transaction): boolean {
  return transaction.startState.facet(editorDarkTheme) !== transaction.state.facet(editorDarkTheme);
}

/**
 * 判断事务是否带来了更完整的语法树（后台解析推进）。
 *
 * @param transaction 事务
 * @returns 语法树变化时为 true
 */
function treeChanged(transaction: Transaction): boolean {
  return syntaxTree(transaction.startState) !== syntaxTree(transaction.state);
}

/**
 * 处理表格节点：始终以表格部件呈现，单元格内直接编辑。
 *
 * @param state 编辑器状态
 * @param node Table 语法节点
 * @param ranges 装饰收集容器
 * @param guards 受保护区域收集容器
 * @returns 是否继续遍历子节点
 */
function collectTable(
  state: EditorState,
  node: SyntaxNodeRef,
  ranges: Range<Decoration>[],
  guards: GuardRange[]
): boolean {
  // 1. 去掉节点末尾可能带上的空白，块级替换必须精确落在行边界
  let end = node.to;
  while (end > node.from && /\s/.test(state.doc.sliceString(end - 1, end))) end -= 1;
  const startLine = state.doc.lineAt(node.from);
  const endLine = state.doc.lineAt(end);
  // 2. 嵌套在引用等容器里的表格不独占整行，退回源码展示
  if (node.from !== startLine.from || end !== endLine.to) return true;
  const model = buildTableModel(state, node.node);
  if (!model) return true;
  const source = state.doc.sliceString(node.from, end);
  ranges.push(Decoration.replace({ widget: new TableWidget(model, source, node.from), block: true }).range(node.from, end));
  guards.push({ kind: "table", from: node.from, to: end, before: node.from, after: end });
  return false;
}

/**
 * 处理围栏代码块节点。
 *
 * @param state 编辑器状态
 * @param node FencedCode 语法节点
 * @param ranges 装饰收集容器
 * @param guards 受保护区域收集容器
 * @param revealable 可显露块收集容器
 * @returns 恒为 false，代码块内部不再深入
 */
function collectFencedCode(
  state: EditorState,
  node: SyntaxNodeRef,
  ranges: Range<Decoration>[],
  guards: GuardRange[],
  revealable: RevealableBlock[]
): boolean {
  const block = analyzeFencedCode(state, node);
  // 1. Mermaid：光标在外时整块渲染为图表
  if (block.language.toLowerCase() === "mermaid" && block.closeLine && block.contentLines.length) {
    revealable.push({ from: node.from, to: node.to });
    const source = block.contentLines.map((line) => line.text).join("\n");
    const dark = state.facet(editorDarkTheme);
    if (!selectionTouches(state, node.from, node.to)) {
      const widget = new MermaidWidget(source, dark, block.contentLines[0].from, false);
      ranges.push(Decoration.replace({ widget, block: true }).range(node.from, block.closeLine.to));
      return false;
    }
    // 光标在块内：照常显示代码，并在块下方挂实时预览
    const widget = new MermaidWidget(source, dark, block.contentLines[0].from, true);
    ranges.push(Decoration.widget({ widget, block: true, side: 1 }).range(block.closeLine.to));
  }
  // 2. 已闭合且有内容时隐藏围栏行，语言标识转为首行角标；
  //    空代码块、未闭合（正在输入）或光标仍在开栏行上时保留围栏，避免正在输入的行突然消失
  const onOpenLine = state.selection.ranges.some((range) => range.head >= block.openLine.from && range.head <= block.openLine.to);
  const hideFences = block.contentLines.length > 0 && block.closeLine !== null && !onOpenLine;
  if (hideFences) {
    const first = block.contentLines[0];
    const last = block.contentLines[block.contentLines.length - 1];
    const open = block.openLine;
    appendHiddenLine(block.openLine, ranges);
    guards.push({ kind: "fence-open", from: open.from, to: open.to, before: open.from > 0 ? open.from - 1 : first.from, after: first.from });
    if (block.closeLine) {
      const close = block.closeLine;
      appendHiddenLine(close, ranges);
      guards.push({ kind: "fence-close", from: close.from, to: close.to, before: last.to, after: close.to < state.doc.length ? close.to + 1 : last.to });
    }
  }
  const visible: Line[] = hideFences
    ? block.contentLines
    : [block.openLine, ...block.contentLines, ...(block.closeLine ? [block.closeLine] : [])];
  visible.forEach((line, index) => {
    const classes = ["cm-md-codeline"];
    if (index === 0) classes.push("cm-md-codeline-first");
    if (index === visible.length - 1) classes.push("cm-md-codeline-last");
    const attributes = index === 0 && hideFences && block.language ? { "data-md-lang": block.language } : undefined;
    ranges.push(Decoration.line({ class: classes.join(" "), attributes }).range(line.from));
  });
  return false;
}

/**
 * 处理块级公式：光标在外时渲染公式，光标在内时显示源码并在下方预览。
 *
 * @param state 编辑器状态
 * @param node BlockMath 语法节点
 * @param ranges 装饰收集容器
 * @param revealable 可显露块收集容器
 * @returns 恒为 false
 */
function collectBlockMath(
  state: EditorState,
  node: SyntaxNodeRef,
  ranges: Range<Decoration>[],
  revealable: RevealableBlock[]
): boolean {
  const startLine = state.doc.lineAt(node.from);
  const endLine = state.doc.lineAt(node.to);
  // 容器内（如引用里）的公式不独占整行，保持源码
  if (node.from !== startLine.from) return false;
  revealable.push({ from: node.from, to: node.to });
  const source = state.doc.sliceString(node.from, node.to);
  const tex = source.replace(/^\$\$/, "").replace(/\$\$\s*$/, "").trim();
  const anchor = Math.min(startLine.to, node.from + 2);
  if (!selectionTouches(state, node.from, node.to)) {
    ranges.push(Decoration.replace({ widget: new BlockMathWidget(tex, anchor, false), block: true }).range(startLine.from, endLine.to));
    return false;
  }
  for (let number = startLine.number; number <= endLine.number; number += 1) {
    ranges.push(Decoration.line({ class: "cm-md-math-line" }).range(state.doc.line(number).from));
  }
  ranges.push(Decoration.widget({ widget: new BlockMathWidget(tex, anchor, true), block: true, side: 1 }).range(endLine.to));
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
