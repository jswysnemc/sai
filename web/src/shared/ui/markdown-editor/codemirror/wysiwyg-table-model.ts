import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

/** 单列的对齐方式；null 表示未显式指定。 */
export type TableAlign = "left" | "center" | "right" | null;

/**
 * 单元格。
 *
 * Lezer 不为空单元格生成 TableCell 节点，因此单元格按竖线分隔符切分，
 * 同时记录去掉首尾空白后的正文区间与两条竖线之间的整个区间。
 */
export type TableCellModel = {
  /** 去掉首尾空白后的原文 */
  text: string;
  /** 正文起点；空单元格时等于 to */
  from: number;
  /** 正文终点 */
  to: number;
  /** 两条竖线之间整个区间的起点，改写单元格时替换这一段 */
  regionFrom: number;
  /** 两条竖线之间整个区间的终点 */
  regionTo: number;
};

/** 一行表格：单元格列表与行起始偏移。 */
export type TableRowModel = {
  cells: TableCellModel[];
  from: number;
  to: number;
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
 * 竖线位置直接取自 Lezer 的 TableDelimiter 节点，
 * 转义竖线等边角情况由解析器处理，不自行按字符切分。
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
  const delimiter = table.getChildren("TableDelimiter").find((node) => node.from > headerNode.to);
  const aligns = delimiter ? parseAligns(state.doc.sliceString(delimiter.from, delimiter.to)) : [];
  const rows = table.getChildren("TableRow").map((row) => rowModel(state, row));
  return { header, aligns, rows };
}

/**
 * 按竖线切分一行的全部单元格。
 *
 * 参数:
 * - `state`: 编辑器状态
 * - `row`: TableHeader 或 TableRow 节点
 *
 * 返回:
 * - 行模型
 */
function rowModel(state: EditorState, row: SyntaxNode): TableRowModel {
  const text = state.doc.sliceString(row.from, row.to);
  const pipes = row.getChildren("TableDelimiter").map((node) => node.from);
  // 1. 竖线把行切成若干段；首段在行首竖线之前、末段在行尾竖线之后，为空时丢弃
  const segments: [number, number][] = [];
  let cursor = row.from;
  for (const pipe of pipes) {
    segments.push([cursor, pipe]);
    cursor = pipe + 1;
  }
  segments.push([cursor, row.to]);
  if (/^\s*\|/.test(text)) segments.shift();
  if (/\|\s*$/.test(text) && segments.length > 0) segments.pop();
  // 2. 每段去掉首尾空白得到正文区间
  const cells = segments.map(([regionFrom, regionTo]) => {
    const raw = state.doc.sliceString(regionFrom, regionTo);
    const lead = raw.length - raw.trimStart().length;
    const body = raw.trim();
    const from = body ? regionFrom + lead : regionFrom + Math.min(1, raw.length);
    return { text: body, from, to: body ? from + body.length : from, regionFrom, regionTo };
  });
  return { cells, from: row.from, to: row.to };
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
