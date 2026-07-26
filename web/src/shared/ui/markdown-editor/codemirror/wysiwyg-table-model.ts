import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

/** 单列的对齐方式；null 表示未显式指定。 */
export type TableAlign = "left" | "center" | "right" | null;

/** 单元格：原文与其在文档中的起始偏移，偏移用于点击定位。 */
export type TableCellModel = {
  text: string;
  from: number;
};

/** 一行表格：单元格列表与行起始偏移。 */
export type TableRowModel = {
  cells: TableCellModel[];
  from: number;
};

/** 整张表格的结构模型。 */
export type TableModel = {
  header: TableRowModel;
  aligns: TableAlign[];
  rows: TableRowModel[];
};

/**
 * 从语法树节点提取表格结构。
 *
 * 直接消费 Lezer 的 Table 节点而不自行按 `|` 切分，
 * 转义竖线、行内代码里的竖线等边角情况都由解析器处理。
 *
 * 参数:
 * - `state`: 编辑器状态
 * - `table`: Table 语法节点
 *
 * 返回:
 * - 表格模型；缺少表头时为 null
 */
export function buildTableModel(state: EditorState, table: SyntaxNode): TableModel | null {
  const headerNode = table.getChild("TableHeader");
  if (!headerNode) return null;
  const header = rowModel(state, headerNode);
  if (!header.cells.length) return null;
  const delimiter = table.getChild("TableDelimiter");
  const aligns = delimiter
    ? parseAligns(state.doc.sliceString(delimiter.from, delimiter.to))
    : [];
  const rows = table.getChildren("TableRow").map((row) => rowModel(state, row));
  return { header, aligns, rows };
}

/**
 * 提取一行内的全部单元格。
 *
 * 参数:
 * - `state`: 编辑器状态
 * - `row`: TableHeader 或 TableRow 节点
 *
 * 返回:
 * - 行模型
 */
function rowModel(state: EditorState, row: SyntaxNode): TableRowModel {
  const cells = row.getChildren("TableCell").map((cell) => ({
    text: state.doc.sliceString(cell.from, cell.to),
    from: cell.from,
  }));
  return { cells, from: row.from };
}

/**
 * 解析分隔行的对齐标记。
 *
 * 参数:
 * - `delimiter`: 分隔行原文，如 `| :--- | :---: |`
 *
 * 返回:
 * - 每列的对齐方式
 */
export function parseAligns(delimiter: string): TableAlign[] {
  return delimiter
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((segment) => {
      const spec = segment.trim();
      const left = spec.startsWith(":");
      const right = spec.endsWith(":") && spec.length > 1;
      if (left && right) return "center";
      if (right) return "right";
      if (left) return "left";
      return null;
    });
}
