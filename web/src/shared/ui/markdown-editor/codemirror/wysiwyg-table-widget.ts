import { EditorView, WidgetType } from "@codemirror/view";
import { appendInlineMarkdown } from "./wysiwyg-inline-format";
import type { TableModel, TableRowModel } from "./wysiwyg-table-model";

/**
 * 表格部件：光标不在表格内时以真实表格呈现源码。
 *
 * 点击任意单元格会把光标送回该单元格的源码位置，
 * 装饰层随即因选区进入表格范围而还原源码，进入编辑态。
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
   * 构建表格元素。
   *
   * @param view 所属编辑器视图
   * @returns 表格容器
   */
  toDOM(view: EditorView) {
    const wrap = document.createElement("div");
    wrap.className = "cm-md-table";
    const table = document.createElement("table");
    const head = document.createElement("thead");
    head.appendChild(this.rowElement(view, this.model.header, "th"));
    table.appendChild(head);
    const body = document.createElement("tbody");
    for (const row of this.model.rows) {
      body.appendChild(this.rowElement(view, row, "td"));
    }
    table.appendChild(body);
    wrap.appendChild(table);
    return wrap;
  }

  /**
   * 构建一行表格。
   *
   * @param view 所属编辑器视图
   * @param row 行模型
   * @param tag 单元格标签，表头用 th，正文用 td
   * @returns 行元素
   */
  private rowElement(view: EditorView, row: TableRowModel, tag: "th" | "td") {
    const tr = document.createElement("tr");
    // 列数以表头为准：缺列补空、多列忽略，与 GFM 渲染规则一致
    const columns = this.model.header.cells.length;
    for (let index = 0; index < columns; index += 1) {
      const cell = row.cells[index];
      const element = document.createElement(tag);
      const align = this.model.aligns[index];
      if (align) element.style.textAlign = align;
      if (cell) appendInlineMarkdown(element, cell.text.trim());
      const anchor = cell ? cell.from : row.from;
      element.addEventListener("mousedown", (event) => {
        // 1. 阻止默认行为，避免编辑器把点击映射到部件外的位置
        event.preventDefault();
        // 2. 光标落到单元格源码处，装饰层随之切回源码编辑态
        view.dispatch({ selection: { anchor } });
        view.focus();
      });
      tr.appendChild(element);
    }
    return tr;
  }

  /**
   * 声明该部件自行处理事件。
   *
   * @returns 恒为 true，点击定位由部件内部完成
   */
  ignoreEvent() {
    return true;
  }
}
