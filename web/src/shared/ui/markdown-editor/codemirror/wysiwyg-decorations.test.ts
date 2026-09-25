import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import type { Decoration } from "@codemirror/view";
import { describe, expect, it } from "vitest";
import { markdownMath } from "./markdown-math-syntax";
import { buildMarkdownDecorations } from "./wysiwyg-decorations";

/**
 * 构建带光标的编辑器状态，语法树预先解析完整。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移
 * @returns 可直接交给装饰构建函数的状态
 */
function stateOf(doc: string, cursor: number) {
  const state = EditorState.create({
    doc,
    selection: { anchor: cursor },
    extensions: [markdown({ base: markdownLanguage, extensions: [markdownMath] })],
  });
  ensureSyntaxTree(state, doc.length, 1000);
  return state;
}

/** 装饰摘要，便于断言。 */
type Digest = { slice: string; className: string; replaced: boolean; widget: string };

/**
 * 汇总文档产生的行内与行级装饰。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移，缺省置于文末
 * @param reveal 是否按光标显露标记
 * @returns 每条装饰的摘要
 */
function digests(doc: string, cursor = doc.length, reveal = true): Digest[] {
  const state = stateOf(doc, cursor);
  const { decorations } = buildMarkdownDecorations(state, [{ from: 0, to: doc.length }], { reveal });
  const result: Digest[] = [];
  const iterator = decorations.iter();
  while (iterator.value) {
    const value: Decoration = iterator.value;
    const spec = value.spec as { class?: string; widget?: object };
    result.push({
      slice: state.doc.sliceString(iterator.from, iterator.to),
      className: spec.class ?? "",
      replaced: value.point && iterator.to > iterator.from,
      widget: spec.widget?.constructor.name ?? "",
    });
    iterator.next();
  }
  return result;
}

/**
 * 取被隐藏（替换）的原文片段。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移
 * @param reveal 是否按光标显露标记
 * @returns 被替换的片段
 */
function hidden(doc: string, cursor = doc.length, reveal = true): string[] {
  return digests(doc, cursor, reveal).filter((item) => item.replaced).map((item) => item.slice);
}

describe("buildMarkdownDecorations", () => {
  it("光标在标题行内也隐藏井号，并给整行附加字号样式", () => {
    const doc = "# 标题\n\n正文";
    expect(hidden(doc, 3)).toContain("# ");
    expect(digests(doc, 3).map((item) => item.className)).toContain("cm-md-hline cm-md-h1-line");
  });

  it("只有井号尚未输入空格时保持原文，避免输入过程中闪烁", () => {
    expect(hidden("#", 1)).not.toContain("#");
  });

  it("光标不在加粗内时隐藏标记，触及时淡色显露", () => {
    const doc = "**重点** 正文";
    expect(hidden(doc).filter((slice) => slice === "**")).toHaveLength(2);
    expect(hidden(doc, 3)).not.toContain("**");
    expect(digests(doc, 3).filter((item) => item.className === "cm-md-syntax")).toHaveLength(2);
  });

  it("编辑器失焦时不显露任何标记", () => {
    expect(hidden("**重点**", 3, false).filter((slice) => slice === "**")).toHaveLength(2);
  });

  it("行内链接只保留可读文字，光标进入时显露地址", () => {
    const doc = "见 [文档](https://example.com) 说明";
    expect(hidden(doc)).toEqual(expect.arrayContaining(["[", "](https://example.com)"]));
    expect(hidden(doc, 5)).not.toContain("](https://example.com)");
  });

  it("无序列表符号替换为圆点，有序列表保留序号", () => {
    expect(digests("- 一项", 0).find((item) => item.slice === "- ")?.widget).toBe("BulletWidget");
    expect(digests("1. 一项", 0).find((item) => item.slice === "1. ")?.widget).toBe("OrderedWidget");
  });

  it("任务项的符号与方框整体替换为勾选框，已完成项加完成样式", () => {
    const all = digests("- [x] 完成项\n\n正文");
    expect(all.find((item) => item.slice === "- [x] ")?.widget).toBe("TaskWidget");
    expect(all.map((item) => item.className)).toContain("cm-md-task-done");
  });

  it("引用行隐藏标记并附加引用竖线样式", () => {
    const all = digests("> 引用\n\n正文");
    expect(hidden("> 引用\n\n正文")).toContain("> ");
    expect(all.some((item) => item.className.includes("cm-md-quote-line"))).toBe(true);
  });

  it("行内公式在光标外渲染为部件，光标进入时显示源码", () => {
    const doc = "质能 $E=mc^2$ 方程";
    expect(digests(doc).find((item) => item.slice === "$E=mc^2$")?.widget).toBe("InlineMathWidget");
    expect(digests(doc, 5).some((item) => item.widget === "InlineMathWidget")).toBe(false);
  });

  it("货币写法不被识别为公式", () => {
    expect(digests("价格 $5 与 $10").some((item) => item.widget === "InlineMathWidget")).toBe(false);
  });

  it("围栏代码块交给块级装饰，行内层不处理", () => {
    expect(hidden("```js\nconst a = 1;\n```\n\n正文")).not.toContain("```");
  });
});
