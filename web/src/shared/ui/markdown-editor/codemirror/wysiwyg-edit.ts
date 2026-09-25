import type { EditorState, TransactionSpec } from "@codemirror/state";

export type MarkdownEdit = {
  from: number;
  to: number;
  insert: string;
  cursor: number;
};

/**
 * 把当前行改成指定级别的标题，已是该级别时去掉标题标记。
 *
 * @param state 编辑器状态
 * @param level 标题级别，1 到 3
 * @returns 替换当前行的编辑
 */
export function setHeading(state: EditorState, level: 1 | 2 | 3): MarkdownEdit {
  const line = state.doc.lineAt(state.selection.main.head);
  const mark = `${"#".repeat(level)} `;
  const stripped = line.text.replace(/^#{1,6} /, "");
  const next = line.text.startsWith(mark) ? stripped : `${mark}${stripped}`;
  return lineEdit(line.from, line.to, next);
}

/**
 * 为当前行增加或减少一级引用。
 *
 * @param state 编辑器状态
 * @param direction 增加或减少
 * @returns 替换当前行的编辑
 */
export function setQuote(state: EditorState, direction: "more" | "less"): MarkdownEdit {
  const line = state.doc.lineAt(state.selection.main.head);
  const match = /^(?:> )*/.exec(line.text);
  const depth = (match?.[0].length ?? 0) / 2;
  const body = line.text.slice(match?.[0].length ?? 0);
  const nextDepth = direction === "more" ? depth + 1 : Math.max(0, depth - 1);
  const next = `${"> ".repeat(nextDepth)}${body}`;
  return lineEdit(line.from, line.to, next);
}

/**
 * 在光标处插入一张 2×2 表格。
 *
 * @param state 编辑器状态
 * @returns 插入编辑
 */
export function insertTable(state: EditorState): MarkdownEdit {
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  const prefix = line.text.trim() ? "\n\n" : "";
  const table = `${prefix}| 列 1 | 列 2 |\n| --- | --- |\n|  |  |\n`;
  return { from: line.to, to: line.to, insert: table, cursor: line.to + table.length };
}

/**
 * 在光标处插入指定语言的代码块。
 *
 * @param state 编辑器状态
 * @param language 围栏语言
 * @returns 插入编辑
 */
export function insertCodeBlock(state: EditorState, language: string): MarkdownEdit {
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  const selected = state.doc.sliceString(state.selection.main.from, state.selection.main.to);
  const body = selected || "";
  const block = `\n\`\`\`${language}\n${body}\n\`\`\`\n`;
  const from = selected ? state.selection.main.from : line.to;
  const to = selected ? state.selection.main.to : line.to;
  return { from, to, insert: block, cursor: from + block.length };
}

/**
 * 切换光标所在代码围栏的语言。
 *
 * @param state 编辑器状态
 * @param language 新语言
 * @returns 围栏行编辑；光标不在代码块内时为 null
 */
export function setCodeLanguage(state: EditorState, language: string): MarkdownEdit | null {
  const fence = openingFence(state);
  if (!fence) return null;
  const next = `\`\`\`${language}`;
  return lineEdit(fence.from, fence.to, next);
}

/**
 * 用成对标记包裹选区；没有选区时插入空标记并把光标放在中间。
 *
 * @param state 编辑器状态
 * @param marker 成对标记，如 **
 * @returns 包裹编辑
 */
export function wrapSelection(state: EditorState, marker: string): MarkdownEdit {
  const range = state.selection.main;
  const selected = state.doc.sliceString(range.from, range.to);
  const insert = `${marker}${selected}${marker}`;
  return {
    from: range.from,
    to: range.to,
    insert,
    cursor: range.from + marker.length + selected.length,
  };
}

/**
 * 切换当前行的列表前缀。
 *
 * @param state 编辑器状态
 * @param kind 无序或有序
 * @returns 替换当前行的编辑
 */
export function toggleList(state: EditorState, kind: "bullet" | "ordered"): MarkdownEdit {
  const line = state.doc.lineAt(state.selection.main.head);
  const prefix = kind === "bullet" ? "- " : "1. ";
  const stripped = line.text.replace(/^(?:[-*] |\d+\. )/, "");
  const next = line.text.startsWith(prefix) ? stripped : `${prefix}${stripped}`;
  return lineEdit(line.from, line.to, next);
}

/**
 * 在光标所在表格行的上方或下方插入空行。
 *
 * @param state 编辑器状态
 * @param where 插入位置
 * @returns 行插入编辑；不在表格内时为 null
 */
export function insertTableRow(state: EditorState, where: "above" | "below"): MarkdownEdit | null {
  const line = tableLine(state);
  if (!line) return null;
  const cells = splitRow(line.text).length;
  const blank = `|${"  |".repeat(Math.max(cells, 1))}`;
  if (where === "above") {
    return { from: line.from, to: line.from, insert: `${blank}\n`, cursor: line.from + 2 };
  }
  return { from: line.to, to: line.to, insert: `\n${blank}`, cursor: line.to + 3 };
}

/**
 * 删除光标所在表格行。分隔行不删。
 *
 * @param state 编辑器状态
 * @returns 删除编辑；不在数据行时为 null
 */
export function deleteTableRow(state: EditorState): MarkdownEdit | null {
  const line = tableLine(state);
  if (!line || isDelimiter(line.text)) return null;
  const from = line.number > 1 ? state.doc.line(line.number - 1).to : line.from;
  const to = line.to;
  return { from, to, insert: "", cursor: from };
}

/**
 * 在表格每一行末尾追加或删除一列。
 *
 * @param state 编辑器状态
 * @param action 追加或删除
 * @returns 整表替换；光标不在表格内时为 null
 */
export function changeTableColumn(state: EditorState, action: "add" | "remove"): MarkdownEdit | null {
  const block = tableBlock(state);
  if (!block) return null;
  const next = block.lines.map((text) => {
    if (isDelimiter(text)) {
      return action === "add" ? `${text.replace(/\s*$/, "")} --- |` : dropLastCell(text, " --- ");
    }
    return action === "add" ? `${text.replace(/\s*$/, "")}  |` : dropLastCell(text, "  ");
  });
  return {
    from: block.from,
    to: block.to,
    insert: next.join("\n"),
    cursor: block.from,
  };
}

/**
 * 把编辑描述转成 CodeMirror 事务。
 *
 * @param edit 编辑描述
 * @returns 事务说明
 */
export function editTransaction(edit: MarkdownEdit): TransactionSpec {
  return {
    changes: { from: edit.from, to: edit.to, insert: edit.insert },
    selection: { anchor: edit.cursor },
  };
}

function lineEdit(from: number, to: number, insert: string): MarkdownEdit {
  return { from, to, insert, cursor: from + insert.length };
}

function openingFence(state: EditorState) {
  const head = state.selection.main.head;
  const current = state.doc.lineAt(head).number;
  let openNumber = 0;
  for (let number = current; number >= 1; number -= 1) {
    if (/^```/.test(state.doc.line(number).text)) {
      openNumber = number;
      break;
    }
  }
  if (!openNumber) return null;
  for (let number = openNumber + 1; number <= state.doc.lines; number += 1) {
    const line = state.doc.line(number);
    if (!/^```/.test(line.text)) continue;
    if (head >= state.doc.line(openNumber).from && head <= line.to) return state.doc.line(openNumber);
    return null;
  }
  return null;
}

function tableLine(state: EditorState) {
  const line = state.doc.lineAt(state.selection.main.head);
  return line.text.includes("|") ? line : null;
}

function isDelimiter(text: string) {
  return /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(text);
}

function splitRow(text: string) {
  return text.trim().replace(/^\|/, "").replace(/\|$/, "").split("|");
}

function dropLastCell(text: string, _filler: string) {
  const cells = splitRow(text);
  if (cells.length <= 1) return text;
  cells.pop();
  return `|${cells.join("|")}|`;
}

function tableBlock(state: EditorState) {
  const current = state.doc.lineAt(state.selection.main.head);
  if (!current.text.includes("|")) return null;
  let start = current.number;
  let end = current.number;
  while (start > 1 && state.doc.line(start - 1).text.includes("|")) start -= 1;
  while (end < state.doc.lines && state.doc.line(end + 1).text.includes("|")) end += 1;
  const lines = [];
  for (let number = start; number <= end; number += 1) lines.push(state.doc.line(number).text);
  return { from: state.doc.line(start).from, to: state.doc.line(end).to, lines };
}
