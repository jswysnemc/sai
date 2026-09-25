import { syntaxTree } from "@codemirror/language";
import { EditorSelection, type EditorState, type StateCommand } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

/** 行内格式：成对标记与对应的语法节点名。 */
export type InlineFormat = {
  marker: string;
  node: string;
};

export const BOLD: InlineFormat = { marker: "**", node: "StrongEmphasis" };
export const ITALIC: InlineFormat = { marker: "*", node: "Emphasis" };
export const STRIKE: InlineFormat = { marker: "~~", node: "Strikethrough" };
export const INLINE_CODE: InlineFormat = { marker: "`", node: "InlineCode" };

/**
 * 找到包含给定区间的指定类型节点。
 *
 * @param state 编辑器状态
 * @param from 区间起点
 * @param to 区间终点
 * @param name 节点名
 * @returns 节点；不存在时为 null
 */
function enclosingNode(state: EditorState, from: number, to: number, name: string): SyntaxNode | null {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(from, 1); node; node = node.parent) {
    if (node.name === name && node.from <= from && node.to >= to) return node;
  }
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(from, -1); node; node = node.parent) {
    if (node.name === name && node.from <= from && node.to >= to) return node;
  }
  return null;
}

/**
 * 切换行内格式，行为对齐 Typora：
 *
 * 1. 光标或选区已在该格式内：去掉这一对标记
 * 2. 有选区：用标记包裹选区，选区保持在原文字上
 * 3. 无选区：插入一对空标记，光标放在中间，接着输入即为该格式
 *
 * @param format 行内格式
 * @returns CodeMirror 状态命令
 */
export function toggleInline(format: InlineFormat): StateCommand {
  return ({ state, dispatch }) => {
    const size = format.marker.length;
    const transaction = state.changeByRange((range) => {
      const existing = enclosingNode(state, range.from, range.to, format.node);
      if (existing) {
        const open = state.doc.sliceString(existing.from, existing.from + size);
        const close = state.doc.sliceString(existing.to - size, existing.to);
        if (open === close && existing.to - existing.from >= size * 2) {
          const changes = state.changes([
            { from: existing.from, to: existing.from + size },
            { from: existing.to - size, to: existing.to },
          ]);
          return { changes, range: range.map(changes) };
        }
      }
      return {
        changes: [
          { from: range.from, insert: format.marker },
          { from: range.to, insert: format.marker },
        ],
        range: EditorSelection.range(range.anchor + size, range.head + size),
      };
    });
    dispatch(state.update(transaction, { scrollIntoView: true, userEvent: "input.format" }));
    return true;
  };
}

/**
 * 插入或改写链接。
 *
 * 有选区时把选区作为链接文字并把光标放进地址括号；
 * 无选区时插入空链接，光标放在文字处。
 *
 * @param target 可自动填入的地址，如剪贴板里的网址
 * @returns CodeMirror 状态命令
 */
export function insertLink(target = ""): StateCommand {
  return ({ state, dispatch }) => {
    const transaction = state.changeByRange((range) => {
      const label = state.doc.sliceString(range.from, range.to);
      const insert = `[${label}](${target})`;
      const cursor = label ? range.from + label.length + 3 + target.length : range.from + 1;
      return { changes: { from: range.from, to: range.to, insert }, range: EditorSelection.cursor(cursor) };
    });
    dispatch(state.update(transaction, { scrollIntoView: true, userEvent: "input.format" }));
    return true;
  };
}

/**
 * 判断文本是否为可直接作为链接地址的网址。
 *
 * @param text 候选文本
 * @returns 单行 http(s) 地址时为 true
 */
export function isLinkUrl(text: string): boolean {
  return /^https?:\/\/\S+$/i.test(text.trim());
}
