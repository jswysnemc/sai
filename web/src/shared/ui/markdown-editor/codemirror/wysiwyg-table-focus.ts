import { ViewPlugin, type EditorView, type ViewUpdate } from "@codemirror/view";
import { wysiwygBlockDecorations } from "./wysiwyg-block-decorations";

/** 单元格坐标：表格起点、行（0 为表头）与列。 */
export type TableCellRef = {
  tableFrom: number;
  row: number;
  column: number;
};

/** 光标在单元格内的落点：开头、末尾或全选。 */
export type CaretPlacement = "start" | "end" | "all";

/** 结构性修改后待聚焦的单元格；表格部件重建完成时消费。 */
let pending: (TableCellRef & { caret: CaretPlacement }) | null = null;

/**
 * 登记一次待聚焦请求，供随后重建的表格部件消费。
 *
 * @param ref 单元格坐标
 * @param caret 光标落点
 * @returns 无
 */
export function requestCellFocus(ref: TableCellRef, caret: CaretPlacement): void {
  pending = { ...ref, caret };
}

/**
 * 表格部件挂载或更新后调用：命中待聚焦请求时聚焦对应单元格。
 *
 * @param wrap 表格部件根元素
 * @param tableFrom 表格起点
 * @returns 无
 */
export function consumeCellFocus(wrap: HTMLElement, tableFrom: number): void {
  if (!pending || pending.tableFrom !== tableFrom) return;
  const request = pending;
  pending = null;
  // 等部件挂进文档后再聚焦
  requestAnimationFrame(() => focusCellElement(wrap, request.row, request.column, request.caret));
}

/**
 * 找到表格部件的根元素。
 *
 * @param view 编辑器视图
 * @param tableFrom 表格起点
 * @returns 根元素；不在视口内时为 null
 */
export function tableElement(view: EditorView, tableFrom: number): HTMLElement | null {
  return view.contentDOM.querySelector<HTMLElement>(`.cm-md-table[data-from="${tableFrom}"]`);
}

/**
 * 聚焦指定单元格并放置光标。
 *
 * @param wrap 表格部件根元素
 * @param row 行下标，0 为表头
 * @param column 列下标
 * @param caret 光标落点
 * @returns 找到并聚焦单元格时为 true
 */
export function focusCellElement(wrap: HTMLElement, row: number, column: number, caret: CaretPlacement): boolean {
  const cell = wrap.querySelector<HTMLElement>(`[data-row="${row}"][data-column="${column}"]`);
  if (!cell) return false;
  cell.focus({ preventScroll: true });
  cell.scrollIntoView({ block: "nearest", inline: "nearest" });
  const selection = window.getSelection();
  if (!selection) return true;
  const range = document.createRange();
  range.selectNodeContents(cell);
  if (caret !== "all") range.collapse(caret === "start");
  selection.removeAllRanges();
  selection.addRange(range);
  return true;
}

/**
 * 光标经键盘移到表格边界时，自动进入单元格。
 *
 * 表格源码是隐藏的，光标停在边界上无处可去：
 * 从上方进入落到表头首格，从下方进入落到末行首格，行为对齐 Typora。
 */
export const tableFocusPlugin = ViewPlugin.fromClass(
  class {
    /**
     * 选区变化时判断是否落在表格边界。
     *
     * @param update 视图更新
     * @returns 无
     */
    update(update: ViewUpdate) {
      if (!update.selectionSet || update.docChanged || !update.view.hasFocus) return;
      if (!update.transactions.some((transaction) => transaction.isUserEvent("select"))) return;
      const selection = update.state.selection.main;
      if (!selection.empty) return;
      const guards = update.state.field(wysiwygBlockDecorations, false)?.guards ?? [];
      const table = guards.find(
        (guard) => guard.kind === "table" && (selection.head === guard.from || selection.head === guard.to)
      );
      if (!table) return;
      const forward = selection.head >= update.startState.selection.main.head;
      const view = update.view;
      requestAnimationFrame(() => {
        const wrap = tableElement(view, table.from);
        if (!wrap) return;
        const rows = wrap.querySelectorAll("tr").length;
        focusCellElement(wrap, forward ? 0 : rows - 1, 0, "start");
      });
    }
  }
);
