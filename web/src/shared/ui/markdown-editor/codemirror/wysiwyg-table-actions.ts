import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import type { EditorView } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import { insertBlock } from "./commands/block-commands";
import { wysiwygBlockDecorations } from "./wysiwyg-block-decorations";
import { createTableSource, matrixOf, serializeTable, type TableMatrix } from "./wysiwyg-table-edit";
import { requestCellFocus, type CaretPlacement, type TableCellRef } from "./wysiwyg-table-focus";
import { buildTableModel, type TableModel } from "./wysiwyg-table-model";

/** 表格在文档中的位置与结构。 */
export type TableLocation = {
  from: number;
  /** 去掉末尾空白后的终点，与块级装饰替换的范围一致 */
  to: number;
  model: TableModel;
};

/**
 * 按起点找到表格并解析结构。
 *
 * @param view 编辑器视图
 * @param tableFrom 表格起点
 * @returns 表格位置与模型；找不到时为 null
 */
export function locateTable(view: EditorView, tableFrom: number): TableLocation | null {
  const state = view.state;
  const tree = ensureSyntaxTree(state, Math.min(state.doc.length, tableFrom + 1), 50) ?? syntaxTree(state);
  let node: SyntaxNode | null = tree.resolveInner(tableFrom, 1);
  while (node && node.name !== "Table") node = node.parent;
  if (!node || node.from !== tableFrom) return null;
  const model = buildTableModel(state, node);
  if (!model) return null;
  let to = node.to;
  while (to > node.from && /\s/.test(state.doc.sliceString(to - 1, to))) to -= 1;
  return { from: node.from, to, model };
}

/**
 * 读取单元格元素对应的坐标。
 *
 * @param element 单元格或其内部元素
 * @returns 单元格坐标；不在表格部件内时为 null
 */
export function cellRefOf(element: Element | null): TableCellRef | null {
  const cell = element?.closest<HTMLElement>("[data-row][data-column]");
  const wrap = cell?.closest<HTMLElement>(".cm-md-table[data-from]");
  if (!cell || !wrap) return null;
  return { tableFrom: Number(wrap.dataset.from), row: Number(cell.dataset.row), column: Number(cell.dataset.column) };
}

/**
 * 对整张表格做结构性修改，并在重建后聚焦指定单元格。
 *
 * @param view 编辑器视图
 * @param tableFrom 表格起点
 * @param edit 矩阵变换
 * @param focus 修改后聚焦的单元格与光标落点；缺省不聚焦
 * @returns 修改成功时为 true
 */
export function editTable(
  view: EditorView,
  tableFrom: number,
  edit: (matrix: TableMatrix) => TableMatrix,
  focus?: { row: number; column: number; caret: CaretPlacement }
): boolean {
  const table = locateTable(view, tableFrom);
  if (!table) return false;
  const next = serializeTable(edit(matrixOf(table.model)));
  if (focus) requestCellFocus({ tableFrom, row: focus.row, column: focus.column }, focus.caret);
  view.dispatch({ changes: { from: table.from, to: table.to, insert: next }, userEvent: "input.table" });
  return true;
}

/**
 * 删除整张表格，光标落到原位置。
 *
 * @param view 编辑器视图
 * @param tableFrom 表格起点
 * @returns 删除成功时为 true
 */
export function deleteTable(view: EditorView, tableFrom: number): boolean {
  const table = locateTable(view, tableFrom);
  if (!table) return false;
  view.dispatch({
    changes: { from: table.from, to: table.to, insert: "" },
    selection: { anchor: table.from },
    userEvent: "input.table",
  });
  view.focus();
  return true;
}

/**
 * 从表格移出光标到上方或下方的正文行；表格位于文档首尾时补一个空行。
 *
 * @param view 编辑器视图
 * @param tableFrom 表格起点
 * @param direction 移出方向
 * @returns 无
 */
export function exitTable(view: EditorView, tableFrom: number, direction: "above" | "below"): void {
  const table = locateTable(view, tableFrom);
  if (!table) return;
  const doc = view.state.doc;
  if (direction === "above") {
    if (table.from > 0) view.dispatch({ selection: { anchor: table.from - 1 }, scrollIntoView: true });
    else view.dispatch({ changes: { from: 0, insert: "\n" }, selection: { anchor: 0 }, userEvent: "input.table" });
  } else if (table.to < doc.length) {
    view.dispatch({ selection: { anchor: table.to + 1 }, scrollIntoView: true });
  } else {
    view.dispatch({
      changes: { from: doc.length, insert: "\n" },
      selection: { anchor: doc.length + 1 },
      userEvent: "input.table",
    });
  }
  view.focus();
}

/**
 * 在光标处插入一张空表格，并聚焦表头首格便于直接输入。
 *
 * @param view 编辑器视图
 * @param columns 列数
 * @param rows 数据行数
 * @param label 表头文字生成函数
 * @returns 恒为 true
 */
export function insertTable(view: EditorView, columns: number, rows: number, label: (index: number) => string): boolean {
  const { spec, blockFrom } = insertBlock(view.state, createTableSource(columns, rows, label), 0);
  // 源码模式没有表格部件，只插入文本，不登记聚焦请求
  if (view.state.field(wysiwygBlockDecorations, false)) requestCellFocus({ tableFrom: blockFrom, row: 0, column: 0 }, "all");
  view.dispatch(spec);
  return true;
}
