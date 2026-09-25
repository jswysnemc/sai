import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";

/** 大纲中的一个标题。 */
export type OutlineHeading = {
  /** 标题级别，1 到 6 */
  level: number;
  /** 去掉语法标记后的可读文字 */
  text: string;
  /** 标题所在行的起始偏移，用于跳转 */
  from: number;
};

/** 整体解析预算（毫秒），超时后退回已有的部分语法树。 */
const PARSE_BUDGET_MS = 40;

/** 标题节点名到级别的映射，覆盖 ATX（#）与 Setext（下划线）两种写法。 */
const HEADING_LEVELS: Record<string, number> = {
  ATXHeading1: 1,
  ATXHeading2: 2,
  ATXHeading3: 3,
  ATXHeading4: 4,
  ATXHeading5: 5,
  ATXHeading6: 6,
  SetextHeading1: 1,
  SetextHeading2: 2,
};

/**
 * 从编辑器状态中提取全部标题。
 *
 * 直接复用 CodeMirror 已增量解析好的 Lezer 语法树，
 * 代码块里的 `#` 行不会被误认成标题。
 *
 * @param state 编辑器状态
 * @returns 按文档顺序排列的标题列表
 */
export function extractOutline(state: EditorState): OutlineHeading[] {
  const tree = ensureSyntaxTree(state, state.doc.length, PARSE_BUDGET_MS) ?? syntaxTree(state);
  const headings: OutlineHeading[] = [];
  tree.iterate({
    enter: (node) => {
      const level = HEADING_LEVELS[node.name];
      if (!level) return true;
      // 1. Setext 标题的下划线行不属于文字，只取首行
      const line = state.doc.lineAt(node.from);
      const raw = state.doc.sliceString(node.from, Math.min(node.to, line.to));
      const text = plainHeadingText(raw);
      if (text) headings.push({ level, text, from: node.from });
      // 2. 标题内部不会再嵌套标题，跳过子节点
      return false;
    },
  });
  return headings;
}

/**
 * 把标题原文转成大纲展示用的纯文字。
 *
 * @param raw 标题行原文，如 `## **重点** 说明 ##`
 * @returns 去掉井号、行内标记与链接地址后的文字
 */
export function plainHeadingText(raw: string): string {
  return raw
    .replace(/^\s{0,3}#{1,6}(?:\s+|$)/, "")
    .replace(/\s+#+\s*$/, "")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/(\*\*|__|~~|\*|_|`)/g, "")
    .trim();
}

/**
 * 找出给定位置所属的标题。
 *
 * @param headings 按文档顺序排列的标题
 * @param position 参考位置，通常是视口顶部对应的文档偏移
 * @returns 位置之前最近一个标题的下标；位置在首个标题之前时为 -1
 */
export function activeHeadingIndex(headings: readonly OutlineHeading[], position: number): number {
  // 二分查找最后一个 from <= position 的标题
  let low = 0;
  let high = headings.length - 1;
  let found = -1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (headings[middle].from <= position) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}

/**
 * 判断两份大纲是否一致，避免无变化时触发重渲染。
 *
 * @param left 旧大纲
 * @param right 新大纲
 * @returns 级别、文字与位置全部相同时为 true
 */
export function sameOutline(left: readonly OutlineHeading[], right: readonly OutlineHeading[]): boolean {
  if (left.length !== right.length) return false;
  return left.every(
    (item, index) =>
      item.level === right[index].level && item.text === right[index].text && item.from === right[index].from
  );
}
