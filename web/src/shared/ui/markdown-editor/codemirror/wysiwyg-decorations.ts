import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, ViewPlugin, type DecorationSet, type EditorView, type ViewUpdate } from "@codemirror/view";
import type { SyntaxNodeRef, Tree } from "@lezer/common";
import { imageUrlResolver } from "./editor-facets";
import { appendLinePrefix, prefixEndOf, type PrefixCache } from "./wysiwyg-line-decorations";
import { InlineMathWidget } from "./wysiwyg-math-widgets";
import { rangeKey, revealedInlineNodes } from "./wysiwyg-reveal";
import { isSyntaxMark, styleClassFor } from "./wysiwyg-syntax-nodes";
import { ImageWidget, RuleWidget } from "./wysiwyg-widgets";

/** 构建结果：可见装饰与需要整体跳过的原子区间。 */
export type MarkdownDecorations = {
  decorations: DecorationSet;
  atomic: DecorationSet;
};

/** 构建选项。 */
export type MarkdownDecorationOptions = {
  /** 是否按光标位置显露行内标记；编辑器失焦时关闭，读起来与成稿一致 */
  reveal?: boolean;
};

/** 单次遍历共享的上下文。 */
type CollectContext = {
  state: EditorState;
  tree: Tree;
  revealed: Set<string>;
  prefixes: PrefixCache;
  ranges: Range<Decoration>[];
  atomic: Range<Decoration>[];
};

/** 显露时的淡色语法标记。 */
const syntaxMark = Decoration.mark({ class: "cm-md-syntax" });

/** 隐藏原文的替换装饰。 */
const hidden = Decoration.replace({});

/**
 * 构建所见即所得行内装饰的视图插件。
 *
 * 行首的 `#`、`>`、列表符号始终隐藏并由行样式呈现；
 * 行内标记只在光标触及所属元素时淡色显露（对齐 Typora），其余时候隐藏。
 */
export const wysiwygDecorations = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    atomic: DecorationSet;

    constructor(view: EditorView) {
      const built = buildMarkdownDecorations(view.state, view.visibleRanges, { reveal: view.hasFocus });
      this.decorations = built.decorations;
      this.atomic = built.atomic;
    }

    /**
     * 在文档、选区、视口或焦点变化时重建装饰。
     *
     * @param update 视图更新
     * @returns 无
     */
    update(update: ViewUpdate) {
      const treeChanged = syntaxTree(update.state) !== syntaxTree(update.startState);
      if (update.docChanged || update.selectionSet || update.viewportChanged || update.focusChanged || treeChanged) {
        const built = buildMarkdownDecorations(update.state, update.view.visibleRanges, {
          reveal: update.view.hasFocus,
        });
        this.decorations = built.decorations;
        this.atomic = built.atomic;
      }
    }
  },
  { decorations: (plugin) => plugin.decorations }
);

/**
 * 遍历指定区域的语法树并收集装饰。
 *
 * 只依赖 EditorState，不触碰视图，便于脱离 DOM 直接验证隐藏与样式规则。
 *
 * @param state 编辑器状态
 * @param visibleRanges 需要处理的区间，通常是视口可见范围
 * @param options 构建选项
 * @returns 排序后的装饰集合与原子区间
 */
export function buildMarkdownDecorations(
  state: EditorState,
  visibleRanges: readonly { from: number; to: number }[],
  options: MarkdownDecorationOptions = {}
): MarkdownDecorations {
  const tree = syntaxTree(state);
  const context: CollectContext = {
    state,
    tree,
    revealed: options.reveal === false ? new Set() : revealedInlineNodes(state, tree),
    prefixes: new Map(),
    ranges: [],
    atomic: [],
  };
  for (const visible of visibleRanges) {
    // 1. 行级：前缀替换、引用竖线与列表悬挂缩进
    for (let position = visible.from; position <= visible.to; ) {
      const line = state.doc.lineAt(position);
      appendLinePrefix(state, line, tree, context.prefixes, context.ranges, context.atomic);
      position = line.to + 1;
    }
    // 2. 行内：标记隐藏、排版样式与部件
    tree.iterate({ from: visible.from, to: visible.to, enter: (node) => collectNode(context, node) });
  }
  // 第二个参数交给 CodeMirror 排序，父子节点的装饰是按遍历序而非位置序产生的
  return { decorations: Decoration.set(context.ranges, true), atomic: Decoration.set(context.atomic, true) };
}

/**
 * 处理单个语法节点。
 *
 * @param context 遍历上下文
 * @param node 语法树节点
 * @returns 是否继续遍历子节点
 */
function collectNode(context: CollectContext, node: SyntaxNodeRef): boolean {
  const { state, ranges } = context;
  const name = node.name;
  // 1. 围栏代码、公式块与表格由块级装饰负责
  if (name === "FencedCode" || name === "BlockMath" || name === "Table") return false;
  // 2. 可整体替换为部件或需要特殊呈现的节点
  if (appendSpecialNode(context, node)) return false;
  // 3. 标题所在行放大字号，空标题也保持标题行高
  appendHeadingLine(context, node);
  // 4. 内容节点附加排版样式
  const styleClass = styleClassFor(name);
  if (styleClass && node.to > node.from) {
    ranges.push(Decoration.mark({ class: styleClass }).range(node.from, node.to));
  }
  // 5. 已完成的任务项淡化并加删除线
  if (name === "Task") appendDoneTask(state, node, ranges);
  // 6. 语法标记：行首标记交给行前缀，其余按所属元素是否显露决定隐藏或淡显
  if (isSyntaxMark(name) && node.to > node.from) {
    if (isLinePrefixMark(context, node)) return false;
    const parent = node.node.parent;
    const revealed = parent ? context.revealed.has(rangeKey(parent.from, parent.to)) : false;
    ranges.push((revealed ? syntaxMark : hidden).range(node.from, node.to));
    return false;
  }
  return true;
}

/**
 * 处理图片、链接、公式与分隔线等需要特殊呈现的节点。
 *
 * @param context 遍历上下文
 * @param node 语法树节点
 * @returns 已处理且不需深入子节点时为 true
 */
function appendSpecialNode(context: CollectContext, node: SyntaxNodeRef): boolean {
  const { state, ranges } = context;
  const revealed = context.revealed.has(rangeKey(node.from, node.to));
  switch (node.name) {
    case "Image":
      return appendImage(context, node, revealed);
    case "Link":
      return appendLink(context, node, revealed);
    case "InlineMath": {
      if (revealed) return false;
      const tex = state.doc.sliceString(node.from + 1, node.to - 1);
      ranges.push(Decoration.replace({ widget: new InlineMathWidget(tex, node.from + 1) }).range(node.from, node.to));
      return true;
    }
    case "HorizontalRule": {
      ranges.push(Decoration.replace({ widget: new RuleWidget() }).range(node.from, node.to));
      context.atomic.push(hidden.range(node.from, node.to));
      return true;
    }
    case "TaskMarker":
    case "ListMark":
      // 行首列表符号与任务框由行前缀替换为部件；不成立的前缀（如只有 `-`）保持原文
      return true;
    default:
      return false;
  }
}

/**
 * 处理图片：光标在外时替换为图片，光标触及时显露源码并把图片挂在其后。
 *
 * @param context 遍历上下文
 * @param node 图片节点
 * @param revealed 是否显露源码
 * @returns 已处理时为 true
 */
function appendImage(context: CollectContext, node: SyntaxNodeRef, revealed: boolean): boolean {
  const { state, ranges } = context;
  const text = state.doc.sliceString(node.from, node.to);
  const src = /\]\(\s*<?([^\s)>]+)>?/.exec(text)?.[1];
  const url = src ? state.facet(imageUrlResolver)(src) : null;
  if (!url) return false;
  const widget = new ImageWidget(url, /!\[([^\]]*)\]/.exec(text)?.[1] ?? "");
  if (revealed) {
    ranges.push(Decoration.mark({ class: "cm-md-image-src" }).range(node.from, node.to));
    ranges.push(Decoration.widget({ widget, side: 1 }).range(node.to));
    return true;
  }
  ranges.push(Decoration.replace({ widget }).range(node.from, node.to));
  return true;
}

/**
 * 处理 `[文字](地址)` 形式的链接。
 *
 * @param context 遍历上下文
 * @param node 链接节点
 * @param revealed 是否显露源码
 * @returns 命中该形式时为 true；自动链接与引用式链接交给通用逻辑
 */
function appendLink(context: CollectContext, node: SyntaxNodeRef, revealed: boolean): boolean {
  const { state, ranges } = context;
  const text = state.doc.sliceString(node.from, node.to);
  const tail = text.lastIndexOf("](");
  if (!text.startsWith("[") || tail <= 0) return false;
  const labelFrom = node.from + 1;
  const labelTo = node.from + tail;
  // 1. 文字部分始终按链接样式呈现，内部的加粗等继续由遍历处理
  if (labelTo > labelFrom) ranges.push(Decoration.mark({ class: "cm-md-link" }).range(labelFrom, labelTo));
  // 2. 显露时方括号与地址淡色可见，否则隐藏
  ranges.push((revealed ? syntaxMark : hidden).range(node.from, labelFrom));
  ranges.push((revealed ? Decoration.mark({ class: "cm-md-syntax cm-md-url" }) : hidden).range(labelTo, node.to));
  // 3. 文字内部可能还有加粗、行内代码，单独遍历这一段
  context.tree.iterate({
    from: labelFrom,
    to: labelTo,
    enter: (child) => (child.from >= labelFrom && child.to <= labelTo ? collectNode(context, child) : true),
  });
  return true;
}

/**
 * 为标题所在行附加行级字号样式。
 *
 * 只有 `#` 而没有后随空白时视为正在输入，不放大，避免输入过程中行高跳动。
 *
 * @param context 遍历上下文
 * @param node 语法树节点
 * @returns 无
 */
function appendHeadingLine(context: CollectContext, node: SyntaxNodeRef): void {
  const match = /^(?:ATX|Setext)Heading(\d)$/.exec(node.name);
  if (!match) return;
  const line = context.state.doc.lineAt(node.from);
  if (/^\s{0,3}#{1,6}$/.test(line.text)) return;
  context.ranges.push(Decoration.line({ class: `cm-md-hline cm-md-h${match[1]}-line` }).range(line.from));
}

/**
 * 为已勾选的任务项正文附加完成样式。
 *
 * @param state 编辑器状态
 * @param node Task 节点
 * @param ranges 装饰收集容器
 * @returns 无
 */
function appendDoneTask(state: EditorState, node: SyntaxNodeRef, ranges: Range<Decoration>[]): void {
  const marker = node.node.getChild("TaskMarker");
  if (!marker || !/x/i.test(state.doc.sliceString(marker.from, marker.to))) return;
  const from = Math.min(marker.to + 1, node.to);
  if (node.to > from) ranges.push(Decoration.mark({ class: "cm-md-task-done" }).range(from, node.to));
}

/**
 * 判断标记是否落在行前缀内（已由行级装饰隐藏）。
 *
 * @param context 遍历上下文
 * @param node 标记节点
 * @returns 位于行前缀内时为 true
 */
function isLinePrefixMark(context: CollectContext, node: SyntaxNodeRef): boolean {
  if (node.name !== "HeaderMark" && node.name !== "QuoteMark") return false;
  const line = context.state.doc.lineAt(node.from);
  const prefixEnd = prefixEndOf(context.state, line, context.tree, context.prefixes);
  // 行首的 # 不成立前缀时（正在输入）保持原文，其余位置的 # 是闭合标记，照常隐藏
  if (node.name === "HeaderMark" && prefixEnd === null && node.node.prevSibling === null && node.node.parent?.name.startsWith("ATX")) {
    return true;
  }
  return prefixEnd !== null && node.from < prefixEnd;
}
