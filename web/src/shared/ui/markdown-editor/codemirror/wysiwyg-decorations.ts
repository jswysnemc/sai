import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import {
  Decoration,
  ViewPlugin,
  type DecorationSet,
  type EditorView,
  type ViewUpdate,
} from "@codemirror/view";
import type { SyntaxNodeRef } from "@lezer/common";
import { isSyntaxMark, styleClassFor } from "./wysiwyg-syntax-nodes";
import { ImageWidget, isRenderableImageUrl, RuleWidget, TaskWidget } from "./wysiwyg-widgets";

/**
 * 构建所见即所得装饰的视图插件。
 *
 * 行为对齐 Typora：光标所在行还原为源码，其余行隐藏语法标记并直接呈现排版。
 * 判定粒度取"行"而非"节点"，因为按节点判定会让同一行的多处标记忽隐忽现。
 */
export const wysiwygDecorations = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildMarkdownDecorations(view.state, view.visibleRanges);
    }

    /**
     * 在文档、选区或视口变化时重建装饰。
     *
     * @param update 视图更新
     * @returns 无
     */
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = buildMarkdownDecorations(update.view.state, update.view.visibleRanges);
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
 * @returns 排序后的装饰集合
 */
export function buildMarkdownDecorations(
  state: EditorState,
  visibleRanges: readonly { from: number; to: number }[]
): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  const activeLines = activeLineNumbers(state);
  for (const visible of visibleRanges) {
    syntaxTree(state).iterate({
      from: visible.from,
      to: visible.to,
      enter: (node) => collectNode(state, node, activeLines, ranges),
    });
  }
  // 第二个参数交给 CodeMirror 排序，父子节点的装饰是按遍历序而非位置序产生的
  return Decoration.set(ranges, true);
}

/**
 * 收集选区覆盖到的行号。
 *
 * @param state 编辑器状态
 * @returns 需要还原源码的行号集合
 */
function activeLineNumbers(state: EditorState): Set<number> {
  const lines = new Set<number>();
  for (const range of state.selection.ranges) {
    const first = state.doc.lineAt(range.from).number;
    const last = state.doc.lineAt(range.to).number;
    for (let line = first; line <= last; line += 1) {
      lines.add(line);
    }
  }
  return lines;
}

/**
 * 处理单个语法节点。
 *
 * @param state 编辑器状态
 * @param node 语法树节点
 * @param activeLines 需要还原源码的行号
 * @param ranges 装饰收集容器
 * @returns 是否继续遍历子节点
 */
function collectNode(
  state: EditorState,
  node: SyntaxNodeRef,
  activeLines: Set<number>,
  ranges: Range<Decoration>[]
): boolean {
  const name = node.name;
  const onActiveLine = activeLines.has(state.doc.lineAt(node.from).number);
  // 1. 可整体替换为部件的节点优先处理，处理后不再深入子节点
  if (!onActiveLine && appendWidget(state, node, name, ranges)) {
    return false;
  }
  // 2. 链接在非光标行只保留可读文字，隐藏 [ 与 ](url)
  if (name === "Link" && !onActiveLine && appendInlineLink(state, node, ranges)) {
    return false;
  }
  // 3. 内容节点附加排版样式
  const styleClass = styleClassFor(name);
  if (styleClass && node.to > node.from) {
    ranges.push(Decoration.mark({ class: styleClass }).range(node.from, node.to));
  }
  // 4. 纯语法标记在非光标行隐藏，块级标记连同其后的分隔空格一起吞掉
  if (isSyntaxMark(name) && !onActiveLine && node.to > node.from && !isFenceMark(node)) {
    const to = BLOCK_MARKS.has(name) ? consumeTrailingSpaces(state, node.to) : node.to;
    ranges.push(Decoration.replace({}).range(node.from, to));
    return false;
  }
  return true;
}

/** 块级标记：隐藏时需连同后随空格一起吞掉，否则正文会多出前导空白。 */
const BLOCK_MARKS = new Set(["HeaderMark", "QuoteMark"]);

/**
 * 从给定位置起跳过同一行内的空格。
 *
 * @param state 编辑器状态
 * @param from 起始偏移
 * @returns 跳过空格后的偏移
 */
function consumeTrailingSpaces(state: EditorState, from: number): number {
  const lineEnd = state.doc.lineAt(from).to;
  let cursor = from;
  while (cursor < lineEnd && state.doc.sliceString(cursor, cursor + 1) === " ") {
    cursor += 1;
  }
  return cursor;
}

/**
 * 为可整体替换的节点追加部件装饰。
 *
 * @param state 编辑器状态
 * @param node 语法树节点
 * @param name 节点名
 * @param ranges 装饰收集容器
 * @returns 已追加部件时为 true
 */
function appendWidget(
  state: EditorState,
  node: SyntaxNodeRef,
  name: string,
  ranges: Range<Decoration>[]
): boolean {
  if (name === "Image") {
    const url = extractLinkUrl(state, node);
    if (!url || !isRenderableImageUrl(url)) return false;
    const widget = new ImageWidget(url, extractImageAlt(state, node));
    ranges.push(Decoration.replace({ widget }).range(node.from, node.to));
    return true;
  }
  if (name === "HorizontalRule") {
    ranges.push(Decoration.replace({ widget: new RuleWidget() }).range(node.from, node.to));
    return true;
  }
  if (name === "TaskMarker") {
    const checked = /x/i.test(state.doc.sliceString(node.from, node.to));
    const widget = new TaskWidget(checked, node.from, node.to);
    ranges.push(Decoration.replace({ widget }).range(node.from, node.to));
    return true;
  }
  return false;
}

/**
 * 为行内链接追加"只显示文字"的装饰。
 *
 * @param state 编辑器状态
 * @param node 链接节点
 * @param ranges 装饰收集容器
 * @returns 命中 `[文字](地址)` 形式时为 true
 */
function appendInlineLink(
  state: EditorState,
  node: SyntaxNodeRef,
  ranges: Range<Decoration>[]
): boolean {
  const text = state.doc.sliceString(node.from, node.to);
  const tail = text.lastIndexOf("](");
  // 自动链接与引用式链接不符合该形式，交给默认的标记隐藏逻辑处理
  if (!text.startsWith("[") || tail <= 0) return false;
  const labelFrom = node.from + 1;
  const labelTo = node.from + tail;
  ranges.push(Decoration.replace({}).range(node.from, labelFrom));
  if (labelTo > labelFrom) {
    ranges.push(Decoration.mark({ class: "cm-md-link" }).range(labelFrom, labelTo));
  }
  ranges.push(Decoration.replace({}).range(labelTo, node.to));
  return true;
}

/**
 * 判断标记是否属于围栏代码块。
 *
 * 围栏代码块的 ``` 不隐藏：隐藏后首尾会各留一个空行，反而比保留标记更难读。
 *
 * @param node 语法树节点
 * @returns 属于围栏代码块时为 true
 */
function isFenceMark(node: SyntaxNodeRef): boolean {
  return node.name === "CodeMark" && node.node.parent?.name === "FencedCode";
}

/**
 * 从链接或图片节点中取出地址。
 *
 * @param state 编辑器状态
 * @param node 链接或图片节点
 * @returns 地址文本；解析不出时为 undefined
 */
function extractLinkUrl(state: EditorState, node: SyntaxNodeRef): string | undefined {
  const text = state.doc.sliceString(node.from, node.to);
  const match = /\]\(\s*<?([^\s)>]+)>?/.exec(text);
  return match?.[1];
}

/**
 * 从图片节点中取出替代文本。
 *
 * @param state 编辑器状态
 * @param node 图片节点
 * @returns 替代文本，缺省为空串
 */
function extractImageAlt(state: EditorState, node: SyntaxNodeRef): string {
  const text = state.doc.sliceString(node.from, node.to);
  return /!\[([^\]]*)\]/.exec(text)?.[1] ?? "";
}
