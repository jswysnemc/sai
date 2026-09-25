import type { TableAlign, TableModel } from "./wysiwyg-table-model";

/** 与语法树无关的表格数据：表头、对齐与数据行，列数以表头为准。 */
export type TableMatrix = {
  header: string[];
  aligns: TableAlign[];
  rows: string[][];
};

/**
 * 把表格模型转成规整的矩阵，缺列补空、多列截断。
 *
 * @param model 表格模型
 * @returns 表格矩阵
 */
export function matrixOf(model: TableModel): TableMatrix {
  const columns = model.header.cells.length;
  const fit = (cells: string[]) => Array.from({ length: columns }, (_, index) => cells[index] ?? "");
  return {
    header: model.header.cells.map((cell) => cell.text),
    aligns: Array.from({ length: columns }, (_, index) => model.aligns[index] ?? null),
    rows: model.rows.map((row) => fit(row.cells.map((cell) => cell.text))),
  };
}

/**
 * 转义单元格文字：未转义的竖线补反斜杠，换行折成空格。
 *
 * @param text 用户输入的文字
 * @returns 可安全写回表格源码的文字
 */
export function escapeCell(text: string): string {
  // 已转义的 \| 原样保留，裸竖线补反斜杠
  return text.replace(/\r?\n/g, " ").replace(/\\?\|/g, (match) => (match === "|" ? "\\|" : match));
}

/**
 * 估算字符串的显示宽度：全角字符按两格计。
 *
 * @param text 字符串
 * @returns 显示宽度
 */
function displayWidth(text: string): number {
  let width = 0;
  for (const char of text) {
    width += /[\u1100-\u115F\u2E80-\uA4CF\uAC00-\uD7A3\uF900-\uFAFF\uFE30-\uFE4F\uFF00-\uFF60\uFFE0-\uFFE6]/u.test(char) ? 2 : 1;
  }
  return width;
}

/**
 * 生成对齐列宽的表格源码，源码模式下同样整齐可读。
 *
 * @param matrix 表格矩阵
 * @returns 不含末尾换行的表格源码
 */
export function serializeTable(matrix: TableMatrix): string {
  const columns = matrix.header.length;
  const widths = Array.from({ length: columns }, (_, index) =>
    Math.max(3, displayWidth(matrix.header[index]), ...matrix.rows.map((row) => displayWidth(row[index] ?? "")))
  );
  const pad = (text: string, index: number) => text + " ".repeat(widths[index] - displayWidth(text));
  const line = (cells: string[]) => `| ${cells.map((cell, index) => pad(cell, index)).join(" | ")} |`;
  const delimiter = widths.map((width, index) => {
    const align = matrix.aligns[index];
    const dashes = "-".repeat(Math.max(3, width - (align === "center" ? 2 : align ? 1 : 0)));
    if (align === "center") return `:${dashes}:`;
    if (align === "left") return `:${dashes}`;
    if (align === "right") return `${dashes}:`;
    return dashes;
  });
  return [line(matrix.header), `| ${delimiter.join(" | ")} |`, ...matrix.rows.map(line)].join("\n");
}

/**
 * 在指定位置插入空数据行。
 *
 * @param matrix 表格矩阵
 * @param index 新行的数据行下标，0 为紧贴表头
 * @returns 新矩阵
 */
export function insertRow(matrix: TableMatrix, index: number): TableMatrix {
  const rows = [...matrix.rows];
  rows.splice(Math.max(0, Math.min(index, rows.length)), 0, matrix.header.map(() => ""));
  return { ...matrix, rows };
}

/**
 * 删除指定数据行。
 *
 * @param matrix 表格矩阵
 * @param index 数据行下标
 * @returns 新矩阵
 */
export function deleteRow(matrix: TableMatrix, index: number): TableMatrix {
  return { ...matrix, rows: matrix.rows.filter((_, rowIndex) => rowIndex !== index) };
}

/**
 * 在指定位置插入空列。
 *
 * @param matrix 表格矩阵
 * @param index 新列下标
 * @returns 新矩阵
 */
export function insertColumn(matrix: TableMatrix, index: number): TableMatrix {
  const at = Math.max(0, Math.min(index, matrix.header.length));
  const splice = <T>(cells: T[], value: T) => [...cells.slice(0, at), value, ...cells.slice(at)];
  return {
    header: splice(matrix.header, ""),
    aligns: splice(matrix.aligns, null),
    rows: matrix.rows.map((row) => splice(row, "")),
  };
}

/**
 * 删除指定列；只剩一列时不删。
 *
 * @param matrix 表格矩阵
 * @param index 列下标
 * @returns 新矩阵
 */
export function deleteColumn(matrix: TableMatrix, index: number): TableMatrix {
  if (matrix.header.length <= 1) return matrix;
  const drop = <T>(cells: T[]) => cells.filter((_, cellIndex) => cellIndex !== index);
  return { header: drop(matrix.header), aligns: drop(matrix.aligns), rows: matrix.rows.map(drop) };
}

/**
 * 设置指定列的对齐方式。
 *
 * @param matrix 表格矩阵
 * @param index 列下标
 * @param align 对齐方式
 * @returns 新矩阵
 */
export function setColumnAlign(matrix: TableMatrix, index: number, align: TableAlign): TableMatrix {
  return { ...matrix, aligns: matrix.aligns.map((current, cellIndex) => (cellIndex === index ? align : current)) };
}

/**
 * 生成一张空表格的源码。
 *
 * @param columns 列数
 * @param rows 数据行数
 * @param label 表头文字生成函数
 * @returns 表格源码
 */
export function createTableSource(columns: number, rows: number, label: (index: number) => string): string {
  return serializeTable({
    header: Array.from({ length: columns }, (_, index) => label(index)),
    aligns: Array.from({ length: columns }, () => null),
    rows: Array.from({ length: rows }, () => Array.from({ length: columns }, () => "")),
  });
}
