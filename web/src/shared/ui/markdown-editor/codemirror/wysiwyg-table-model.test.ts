import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { buildTableModel, parseAligns, type TableModel } from "./wysiwyg-table-model";

/**
 * 解析文档中的第一张表格。
 *
 * @param doc 文档内容
 * @returns 表格模型；找不到表格时为 null
 */
function tableOf(doc: string): TableModel | null {
  const state = EditorState.create({
    doc,
    extensions: [markdown({ base: markdownLanguage })],
  });
  const tree = ensureSyntaxTree(state, state.doc.length, 1000);
  if (!tree) return null;
  let model: TableModel | null = null;
  tree.iterate({
    enter: (node) => {
      if (node.name !== "Table" || model) return true;
      model = buildTableModel(state, node.node);
      return false;
    },
  });
  return model;
}

describe("buildTableModel", () => {
  it("提取表头、数据行与单元格偏移", () => {
    const doc = "| 名称 | 数量 |\n| --- | --- |\n| 苹果 | 3 |";
    const model = tableOf(doc);
    expect(model).not.toBeNull();
    expect(model?.header.cells.map((cell) => cell.text.trim())).toEqual(["名称", "数量"]);
    expect(model?.rows).toHaveLength(1);
    expect(model?.rows[0].cells.map((cell) => cell.text.trim())).toEqual(["苹果", "3"]);
    // 单元格偏移应指向源码中的对应位置，供点击定位使用
    const from = model?.rows[0].cells[0].from ?? 0;
    expect(doc.slice(from, from + 2)).toBe("苹果");
  });

  it("数据行缺列时模型保持原样，由部件负责补空", () => {
    const doc = "| a | b | c |\n| --- | --- | --- |\n| 1 |";
    const model = tableOf(doc);
    expect(model?.header.cells).toHaveLength(3);
    expect(model?.rows[0].cells.length).toBeLessThanOrEqual(3);
  });
});

describe("parseAligns", () => {
  it("识别左中右对齐与未指定", () => {
    expect(parseAligns("| :--- | :---: | ---: | --- |")).toEqual([
      "left",
      "center",
      "right",
      null,
    ]);
  });

  it("兼容省略首尾竖线的写法", () => {
    expect(parseAligns(":--- | ---:")).toEqual(["left", "right"]);
  });
});
