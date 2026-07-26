import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { buildMarkdownDecorations } from "./wysiwyg-decorations";

/**
 * 构建带光标的编辑器状态。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移，默认置于文末
 * @returns 可直接交给装饰构建函数的状态
 */
function stateOf(doc: string, cursor = doc.length) {
  return EditorState.create({
    doc,
    selection: { anchor: cursor },
    extensions: [markdown({ base: markdownLanguage })],
  });
}

/**
 * 收集文档中被隐藏的片段。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移
 * @returns 所有被替换掉的原文片段
 */
function hiddenSlices(doc: string, cursor: number): string[] {
  const state = stateOf(doc, cursor);
  const decorations = buildMarkdownDecorations(state, [{ from: 0, to: doc.length }]);
  const slices: string[] = [];
  const iterator = decorations.iter();
  while (iterator.value) {
    // point 装饰即 replace，mark 装饰不隐藏内容
    if (iterator.value.point || iterator.value.spec.widget) {
      slices.push(state.doc.sliceString(iterator.from, iterator.to));
    }
    iterator.next();
  }
  return slices;
}

/**
 * 收集文档中命中的排版样式类。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移
 * @returns 所有 mark 装饰的类名
 */
function styleClasses(doc: string, cursor: number): string[] {
  const state = stateOf(doc, cursor);
  const decorations = buildMarkdownDecorations(state, [{ from: 0, to: doc.length }]);
  const classes: string[] = [];
  const iterator = decorations.iter();
  while (iterator.value) {
    const className = iterator.value.spec.class;
    if (typeof className === "string") classes.push(className);
    iterator.next();
  }
  return classes;
}

describe("buildMarkdownDecorations", () => {
  it("在非光标行隐藏标题标记", () => {
    const doc = "# 标题\n\n正文";
    // 光标停在最后的正文行
    expect(hiddenSlices(doc, doc.length)).toContain("# ");
  });

  it("光标进入该行时还原标题标记", () => {
    const doc = "# 标题\n\n正文";
    expect(hiddenSlices(doc, 2)).not.toContain("# ");
  });

  it("非光标行隐藏加粗标记并保留加粗样式", () => {
    const doc = "**重点**\n\n正文";
    expect(hiddenSlices(doc, doc.length).filter((slice) => slice === "**")).toHaveLength(2);
    expect(styleClasses(doc, doc.length)).toContain("cm-md-strong");
  });

  it("行内链接只保留可读文字", () => {
    const doc = "见 [文档](https://example.com) 说明\n\n正文";
    const hidden = hiddenSlices(doc, doc.length);
    expect(hidden).toContain("[");
    expect(hidden).toContain("](https://example.com)");
    expect(styleClasses(doc, doc.length)).toContain("cm-md-link");
  });

  it("围栏代码块保留 ``` 标记", () => {
    const doc = "```js\nconst a = 1;\n```\n\n正文";
    expect(hiddenSlices(doc, doc.length)).not.toContain("```");
  });

  it("标题始终附带分级样式", () => {
    const doc = "## 二级\n\n正文";
    expect(styleClasses(doc, doc.length)).toContain("cm-md-heading cm-md-h2");
  });

  it("任务列表在非光标行替换为勾选框", () => {
    const doc = "- [x] 完成项\n\n正文";
    expect(hiddenSlices(doc, doc.length)).toContain("[x]");
  });
});
