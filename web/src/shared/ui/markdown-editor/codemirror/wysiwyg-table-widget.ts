import { EditorView, WidgetType } from "@codemirror/view";
import { appendInlineMarkdown } from "./wysiwyg-inline-format";
import { editTable } from "./wysiwyg-table-actions";
import { escapeCell } from "./wysiwyg-table-edit";
import { consumeCellFocus } from "./wysiwyg-table-focus";
import { handleCellKey } from "./wysiwyg-table-keys";
import type { TableCellModel, TableModel } from "./wysiwyg-table-model";

/** 部件根元素上挂载的最新模型，事件处理统一从这里读取，部件就地更新时只需替换它。 */
const hosts = new WeakMap<HTMLElement, { model: TableModel; from: number }>();

/**
 * 取某个单元格的模型；数据行缺列时为 undefined。
 *
 * @param model 表格模型
 * @param row 行下标，0 为表头
 * @param column 列下标
 * @returns 单元格模型
 */
function cellModel(model: TableModel, row: number, column: number): TableCellModel | undefined {
  return (row === 0 ? model.header : model.rows[row - 1])?.cells[column];
}

/**
 * 判断单元格当前的编辑内容与源码是否一致。
 *
 * 用户输入里的裸竖线、首尾空白在写回源码时会被规范化，比较前按同样规则处理，
 * 否则每次输入都会被判定为不一致而重置内容，打断光标。
 *
 * @param element 单元格元素
 * @param text 源码中的单元格原文
 * @returns 一致时为 true
 */
function matchesSource(element: HTMLElement, text: string): boolean {
  return escapeCell(element.textContent ?? "").trim() === text;
}

/**
 * 按是否聚焦渲染单元格：聚焦时显示原文便于编辑，失焦时显示排版结果。
 *
 * @param element 单元格元素
 * @param text 单元格原文
 * @param editing 是否处于编辑态
 * @returns 无
 */
function renderCell(element: HTMLElement, text: string, editing: boolean): void {
  element.dataset.text = text;
  element.textContent = "";
  if (editing) element.textContent = text.replace(/\\\|/g, "|");
  else appendInlineMarkdown(element, text.replace(/\\\|/g, "|"));
}

/**
 * 表格部件：始终以真实表格呈现，单元格就地编辑。
 *
 * 1. 输入即写回：每次输入只替换该单元格两条竖线之间的源码，撤销历史与源码模式实时一致
 * 2. 就地更新：写回后部件经 updateDOM 复用原有 DOM，焦点与光标不受打断
 * 3. 聚焦显示原文、失焦显示排版，含格式的单元格也能精确编辑
 */
export class TableWidget extends WidgetType {
  constructor(
    private readonly model: TableModel,
    private readonly source: string,
    private readonly from: number
  ) {
    super();
  }

  /**
   * 判断两个部件是否等价，避免重复重建 DOM。
   *
   * @param other 另一个部件
   * @returns 源码与位置都相同时为 true
   */
  eq(other: TableWidget) {
    return other.source === this.source && other.from === this.from;
  }

  /**
   * 构建表格元素并绑定编辑事件。
   *
   * @param view 所属编辑器视图
   * @returns 表格容器
   */
  toDOM(view: EditorView) {
    const wrap = document.createElement("div");
    wrap.className = "cm-md-table";
    const table = document.createElement("table");
    const head = document.createElement("thead");
    head.appendChild(this.rowElement(0, "th"));
    table.appendChild(head);
    const body = document.createElement("tbody");
    this.model.rows.forEach((_, index) => body.appendChild(this.rowElement(index + 1, "td")));
    table.appendChild(body);
    wrap.appendChild(table);
    this.attach(wrap);
    bindTableEvents(wrap, view);
    consumeCellFocus(wrap, this.from);
    return wrap;
  }

  /**
   * 结构不变时就地更新：替换模型，刷新内容有变化的单元格。
   *
   * @param dom 旧部件的容器
   * @returns 行列数一致时为 true
   */
  updateDOM(dom: HTMLElement) {
    const rows = dom.querySelectorAll("tr");
    const columns = this.model.header.cells.length;
    if (!hosts.has(dom) || rows.length !== this.model.rows.length + 1) return false;
    if (rows[0].querySelectorAll("[data-column]").length !== columns) return false;
    this.attach(dom);
    rows.forEach((row, rowIndex) => {
      row.querySelectorAll<HTMLElement>("[data-column]").forEach((element) => {
        const column = Number(element.dataset.column);
        const text = cellModel(this.model, rowIndex, column)?.text ?? "";
        element.style.textAlign = this.model.aligns[column] ?? "";
        const editing = element === document.activeElement;
        // 1. 编辑中的单元格只在源码被外部改动（如撤销）时重置，输入过程中保持原样
        if (editing && matchesSource(element, text)) return;
        if (!editing && element.dataset.text === text) return;
        renderCell(element, text, editing);
        if (editing) placeCaretAtEnd(element);
      });
    });
    consumeCellFocus(dom, this.from);
    return true;
  }

  /**
   * 把最新模型登记到根元素上。
   *
   * @param wrap 表格容器
   * @returns 无
   */
  private attach(wrap: HTMLElement): void {
    wrap.dataset.from = String(this.from);
    hosts.set(wrap, { model: this.model, from: this.from });
  }

  /**
   * 构建一行表格；列数以表头为准，缺列补空、多列忽略，与 GFM 渲染规则一致。
   *
   * @param row 行下标，0 为表头
   * @param tag 单元格标签
   * @returns 行元素
   */
  private rowElement(row: number, tag: "th" | "td") {
    const tr = document.createElement("tr");
    this.model.header.cells.forEach((_, column) => {
      const element = document.createElement(tag);
      element.dataset.row = String(row);
      element.dataset.column = String(column);
      element.contentEditable = "true";
      element.spellcheck = false;
      element.style.textAlign = this.model.aligns[column] ?? "";
      renderCell(element, cellModel(this.model, row, column)?.text ?? "", false);
      tr.appendChild(element);
    });
    return tr;
  }

  /**
   * 渲染前的估算高度，减少首次测量时的滚动跳动。
   *
   * @returns 估算高度（像素）
   */
  get estimatedHeight() {
    return (this.model.rows.length + 1) * 30 + 8;
  }

  /**
   * 声明该部件自行处理事件。
   *
   * @returns 恒为 true，编辑与导航由部件内部完成
   */
  ignoreEvent() {
    return true;
  }
}

/**
 * 把光标放到元素末尾。
 *
 * @param element 目标元素
 * @returns 无
 */
function placeCaretAtEnd(element: HTMLElement): void {
  const selection = window.getSelection();
  if (!selection) return;
  const range = document.createRange();
  range.selectNodeContents(element);
  range.collapse(false);
  selection.removeAllRanges();
  selection.addRange(range);
}

/**
 * 读取事件所在的单元格及其坐标。
 *
 * @param wrap 表格容器
 * @param target 事件目标
 * @returns 单元格元素与坐标；不在单元格内时为 null
 */
function cellAt(wrap: HTMLElement, target: EventTarget | null) {
  const element = target instanceof Element ? target.closest<HTMLElement>("[data-row][data-column]") : null;
  const host = hosts.get(wrap);
  if (!element || !host) return null;
  return { element, host, row: Number(element.dataset.row), column: Number(element.dataset.column) };
}

/**
 * 把单元格内容写回源码。
 *
 * @param view 编辑器视图
 * @param wrap 表格容器
 * @param element 单元格元素
 * @returns 无
 */
function commitCell(view: EditorView, wrap: HTMLElement, element: HTMLElement): void {
  const cell = cellAt(wrap, element);
  if (!cell || !view.state.facet(EditorView.editable)) return;
  const text = escapeCell(element.textContent ?? "").trim();
  const model = cellModel(cell.host.model, cell.row, cell.column);
  if (model?.text === text) return;
  element.dataset.text = text;
  // 1. 常规情况：只替换两条竖线之间的这一段
  if (model) {
    view.dispatch({
      changes: { from: model.regionFrom, to: model.regionTo, insert: ` ${text} ` },
      userEvent: "input.table",
    });
    return;
  }
  // 2. 数据行缺列：整表规整后再写入
  editTable(view, cell.host.from, (matrix) => {
    const rows = matrix.rows.map((row, index) =>
      index === cell.row - 1 ? row.map((value, column) => (column === cell.column ? text : value)) : row
    );
    return { ...matrix, rows };
  });
}

/**
 * 以事件委托方式绑定单元格的编辑、导航与粘贴行为。
 *
 * @param wrap 表格容器
 * @param view 编辑器视图
 * @returns 无
 */
function bindTableEvents(wrap: HTMLElement, view: EditorView): void {
  let composing = false;
  wrap.addEventListener("focusin", (event) => {
    const cell = cellAt(wrap, event.target);
    // 含格式的单元格聚焦时切为原文；纯文字单元格内容相同，不重建以保留点击处的光标
    if (cell && cell.element.textContent !== (cell.element.dataset.text ?? "").replace(/\\\|/g, "|")) {
      renderCell(cell.element, cell.element.dataset.text ?? "", true);
      placeCaretAtEnd(cell.element);
    }
  });
  wrap.addEventListener("focusout", (event) => {
    const cell = cellAt(wrap, event.target);
    if (!cell) return;
    commitCell(view, wrap, cell.element);
    renderCell(cell.element, cell.element.dataset.text ?? "", false);
  });
  wrap.addEventListener("compositionstart", () => {
    composing = true;
  });
  wrap.addEventListener("compositionend", (event) => {
    composing = false;
    const cell = cellAt(wrap, event.target);
    if (cell) commitCell(view, wrap, cell.element);
  });
  wrap.addEventListener("input", (event) => {
    const cell = cellAt(wrap, event.target);
    if (cell && !composing) commitCell(view, wrap, cell.element);
  });
  wrap.addEventListener("keydown", (event) => {
    const cell = cellAt(wrap, event.target);
    if (!cell || composing || event.isComposing) return;
    // 先写回当前单元格，结构性操作基于最新源码
    commitCell(view, wrap, cell.element);
    if (handleCellKey(event, view, wrap, { tableFrom: cell.host.from, row: cell.row, column: cell.column })) {
      event.preventDefault();
    }
  });
  wrap.addEventListener("paste", (event) => {
    // 只接受纯文本，换行折成空格，避免把 HTML 或多行内容塞进单元格
    event.preventDefault();
    const text = event.clipboardData?.getData("text/plain").replace(/\r?\n/g, " ") ?? "";
    document.execCommand("insertText", false, text);
  });
}
