import { redo, undo } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";
import { deleteRow, insertRow } from "./wysiwyg-table-edit";
import { editTable, exitTable } from "./wysiwyg-table-actions";
import { focusCellElement, type TableCellRef } from "./wysiwyg-table-focus";

/**
 * 读取单元格内光标相对正文的位置。
 *
 * @param cell 单元格元素
 * @returns 光标是否在开头、末尾，以及是否有选中文字
 */
function caretInfo(cell: HTMLElement): { atStart: boolean; atEnd: boolean; collapsed: boolean } {
  const selection = window.getSelection();
  if (!selection || !selection.rangeCount) return { atStart: true, atEnd: true, collapsed: true };
  const range = selection.getRangeAt(0);
  const before = document.createRange();
  before.selectNodeContents(cell);
  before.setEnd(range.startContainer, range.startOffset);
  const after = document.createRange();
  after.selectNodeContents(cell);
  after.setStart(range.endContainer, range.endOffset);
  return {
    atStart: before.toString().length === 0,
    atEnd: after.toString().length === 0,
    collapsed: range.collapsed,
  };
}

/**
 * 用成对标记包裹单元格内的选中文字。
 *
 * @param marker 成对标记，如 **
 * @returns 无
 */
function wrapCellSelection(marker: string): void {
  const text = window.getSelection()?.toString() ?? "";
  // execCommand 会触发 input 事件，改动随之写回源码并进入撤销历史
  document.execCommand("insertText", false, `${marker}${text}${marker}`);
}

/**
 * 处理单元格内的按键：在单元格之间导航、增删行、移出表格与撤销重做。
 *
 * @param event 键盘事件
 * @param view 编辑器视图
 * @param wrap 表格部件根元素
 * @param ref 当前单元格坐标
 * @returns 已处理时为 true，调用方随即阻止默认行为
 */
export function handleCellKey(event: KeyboardEvent, view: EditorView, wrap: HTMLElement, ref: TableCellRef): boolean {
  const rows = wrap.querySelectorAll("tr").length;
  const columns = wrap.querySelectorAll("tr:first-child [data-column]").length;
  const cell = event.target as HTMLElement;
  const mod = event.metaKey || event.ctrlKey;
  const move = (row: number, column: number, caret: "start" | "end" | "all") => focusCellElement(wrap, row, column, caret);
  const addRowBelow = () =>
    editTable(view, ref.tableFrom, (matrix) => insertRow(matrix, ref.row), {
      row: ref.row + 1,
      column: ref.column,
      caret: "start",
    });
  // 1. 撤销与重做交给编辑器历史，单元格内容随部件更新同步
  if (mod && event.key.toLowerCase() === "z") return event.shiftKey ? redo(view) : undo(view);
  if (mod && event.key.toLowerCase() === "y") return redo(view);
  // 2. 单元格内的行内格式
  if (mod && !event.shiftKey && (event.key === "b" || event.key === "i")) {
    wrapCellSelection(event.key === "b" ? "**" : "*");
    return true;
  }
  if (mod && event.key === "Enter") return addRowBelow();
  if (mod && event.shiftKey && event.key === "Backspace" && ref.row > 0) {
    return editTable(view, ref.tableFrom, (matrix) => deleteRow(matrix, ref.row - 1), {
      row: Math.min(ref.row, rows - 2),
      column: ref.column,
      caret: "end",
    });
  }
  const caret = caretInfo(cell);
  switch (event.key) {
    case "Tab": {
      // 3. Tab 顺序走格，末格再按一次追加新行
      const index = ref.row * columns + ref.column + (event.shiftKey ? -1 : 1);
      if (index < 0) return true;
      if (index >= rows * columns) return addRowBelow();
      return move(Math.floor(index / columns), index % columns, "all");
    }
    case "Enter":
      if (event.shiftKey) return true;
      // 4. Enter 走到下一行同列，末行时追加新行
      if (ref.row + 1 < rows) return move(ref.row + 1, ref.column, "end");
      return addRowBelow();
    case "ArrowUp":
      if (ref.row > 0) return move(ref.row - 1, ref.column, "end");
      exitTable(view, ref.tableFrom, "above");
      return true;
    case "ArrowDown":
      if (ref.row + 1 < rows) return move(ref.row + 1, ref.column, "end");
      exitTable(view, ref.tableFrom, "below");
      return true;
    case "ArrowLeft":
      if (!caret.collapsed || !caret.atStart || event.shiftKey) return false;
      if (ref.column > 0) return move(ref.row, ref.column - 1, "end");
      if (ref.row > 0) return move(ref.row - 1, columns - 1, "end");
      exitTable(view, ref.tableFrom, "above");
      return true;
    case "ArrowRight":
      if (!caret.collapsed || !caret.atEnd || event.shiftKey) return false;
      if (ref.column + 1 < columns) return move(ref.row, ref.column + 1, "start");
      if (ref.row + 1 < rows) return move(ref.row + 1, 0, "start");
      exitTable(view, ref.tableFrom, "below");
      return true;
    case "Escape":
      exitTable(view, ref.tableFrom, "below");
      return true;
    default:
      return false;
  }
}
