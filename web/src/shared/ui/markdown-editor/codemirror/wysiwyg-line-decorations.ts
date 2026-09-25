import type { EditorState, Line, Range } from "@codemirror/state";
import { Decoration } from "@codemirror/view";
import type { Tree } from "@lezer/common";
import { analyzeLinePrefix, type LinePrefix } from "./wysiwyg-line-prefix";
import { BulletWidget, OrderedWidget, TaskWidget } from "./wysiwyg-list-widgets";

/** 单次构建内的行前缀缓存，键为行号。 */
export type PrefixCache = Map<number, LinePrefix | null>;

/**
 * 读取（必要时计算）一行的前缀。
 *
 * @param state 编辑器状态
 * @param line 目标行
 * @param tree 语法树
 * @param cache 行前缀缓存
 * @returns 行前缀；无需处理时为 null
 */
export function prefixOf(state: EditorState, line: Line, tree: Tree, cache: PrefixCache): LinePrefix | null {
  if (!cache.has(line.number)) cache.set(line.number, analyzeLinePrefix(state, line, tree));
  return cache.get(line.number) ?? null;
}

/**
 * 读取一行隐藏前缀的结束位置。
 *
 * @param state 编辑器状态
 * @param line 目标行
 * @param tree 语法树
 * @param cache 行前缀缓存
 * @returns 前缀结束偏移；该行没有前缀时为 null
 */
export function prefixEndOf(state: EditorState, line: Line, tree: Tree, cache: PrefixCache): number | null {
  return prefixOf(state, line, tree, cache)?.to ?? null;
}

/**
 * 为一行追加前缀相关的装饰。
 *
 * 1. 行样式：通过 CSS 变量传入引用层数与列表层数，由样式统一计算缩进与竖线
 * 2. 前缀替换：列表符号换成圆点、序号或勾选框，标题与引用标记直接隐藏
 * 3. 原子区间：连同上一行的换行符一起，左右移动光标时一步跨过隐藏前缀
 *
 * @param state 编辑器状态
 * @param line 目标行
 * @param tree 语法树
 * @param cache 行前缀缓存
 * @param ranges 装饰收集容器
 * @param atomic 原子区间收集容器
 * @returns 无
 */
export function appendLinePrefix(
  state: EditorState,
  line: Line,
  tree: Tree,
  cache: PrefixCache,
  ranges: Range<Decoration>[],
  atomic: Range<Decoration>[]
): void {
  const prefix = prefixOf(state, line, tree, cache);
  if (!prefix) return;
  const marker = prefix.marker;
  const listItem = marker !== null && marker.kind !== "heading";
  // 1. 行样式
  if (prefix.quoteDepth > 0 || prefix.listDepth > 0) {
    const classes = ["cm-md-block-line"];
    if (prefix.quoteDepth > 0) classes.push("cm-md-quote-line");
    if (listItem) classes.push("cm-md-li");
    ranges.push(
      Decoration.line({
        class: classes.join(" "),
        attributes: { style: `--md-quote: ${prefix.quoteDepth}; --md-list: ${prefix.listDepth}` },
      }).range(line.from)
    );
  }
  if (prefix.to <= prefix.from) return;
  // 2. 前缀替换
  const widget = markerWidget(prefix, state);
  ranges.push((widget ? Decoration.replace({ widget }) : Decoration.replace({})).range(prefix.from, prefix.to));
  // 3. 原子区间
  const atomicFrom = line.number > 1 ? prefix.from - 1 : prefix.from;
  atomic.push(Decoration.replace({}).range(atomicFrom, prefix.to));
}

/**
 * 按行首元素选择替换部件。
 *
 * @param prefix 行前缀
 * @param state 编辑器状态
 * @returns 部件；标题、引用与续行只需隐藏时为 null
 */
function markerWidget(prefix: LinePrefix, state: EditorState) {
  const marker = prefix.marker;
  if (!marker) return null;
  switch (marker.kind) {
    case "bullet":
      return new BulletWidget(Math.max(1, prefix.listDepth));
    case "ordered":
      return new OrderedWidget(marker.label);
    case "task":
      return new TaskWidget(/x/i.test(state.doc.sliceString(marker.markerFrom, marker.markerTo)), marker.markerFrom, marker.markerTo);
    default:
      return null;
  }
}
