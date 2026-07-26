import type { EditorState, Line } from "@codemirror/state";
import type { SyntaxNodeRef } from "@lezer/common";

/** 围栏代码块的行级结构。 */
export type FencedCodeBlock = {
  /** 语言标识，来自开栏后的 info 字符串，可能为空 */
  language: string;
  /** 开栏行（```lang） */
  openLine: Line;
  /** 闭栏行；未闭合（正在输入）时为 null */
  closeLine: Line | null;
  /** 围栏之间的内容行，不含围栏行本身 */
  contentLines: Line[];
};

/**
 * 解析围栏代码块的行级结构。
 *
 * 参数:
 * - `state`: 编辑器状态
 * - `node`: FencedCode 语法节点
 *
 * 返回:
 * - 开栏行、闭栏行、内容行与语言标识
 */
export function analyzeFencedCode(state: EditorState, node: SyntaxNodeRef): FencedCodeBlock {
  const openLine = state.doc.lineAt(node.from);
  const lastLine = state.doc.lineAt(node.to);
  // 1. 闭合判定：两个 CodeMark 且末个落在最后一行，才存在独立的闭栏行
  const marks = node.node.getChildren("CodeMark");
  const lastMark = marks[marks.length - 1];
  const closed =
    marks.length >= 2 &&
    lastMark !== undefined &&
    lastLine.number > openLine.number &&
    state.doc.lineAt(lastMark.from).number === lastLine.number;
  const closeLine = closed ? lastLine : null;
  // 2. 语言标识取 CodeInfo 节点原文
  const info = node.node.getChild("CodeInfo");
  const language = info ? state.doc.sliceString(info.from, info.to).trim() : "";
  // 3. 内容行为围栏之间的行；未闭合时最后一行也是内容
  const contentEnd = closed ? lastLine.number - 1 : lastLine.number;
  const contentLines: Line[] = [];
  for (let number = openLine.number + 1; number <= contentEnd; number += 1) {
    contentLines.push(state.doc.line(number));
  }
  return { language, openLine, closeLine, contentLines };
}
