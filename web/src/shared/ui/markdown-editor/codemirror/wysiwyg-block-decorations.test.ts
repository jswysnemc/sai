import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { markdownMath } from "./markdown-math-syntax";
import { buildBlockDecorations } from "./wysiwyg-block-decorations";

/**
 * 构建带光标的编辑器状态。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移，默认置于文末
 * @returns 编辑器状态
 */
function stateOf(doc: string, cursor = doc.length) {
  return EditorState.create({
    doc,
    selection: { anchor: cursor },
    extensions: [markdown({ base: markdownLanguage, extensions: [markdownMath] })],
  });
}

/** 收集到的装饰摘要，便于断言。 */
type DecorationDigest = {
  slice: string;
  widget: boolean;
  line: boolean;
  className: string;
  lang: string | undefined;
};

/**
 * 汇总文档产生的块级装饰。
 *
 * @param doc 文档内容
 * @param cursor 光标偏移
 * @returns 每条装饰的摘要
 */
function digests(doc: string, cursor: number): DecorationDigest[] {
  const state = stateOf(doc, cursor);
  const decorations = buildBlockDecorations(state);
  const result: DecorationDigest[] = [];
  const iterator = decorations.iter();
  while (iterator.value) {
    const spec = iterator.value.spec as {
      widget?: unknown;
      class?: string;
      attributes?: Record<string, string>;
    };
    result.push({
      slice: state.doc.sliceString(iterator.from, iterator.to),
      widget: spec.widget !== undefined,
      line: iterator.from === iterator.to,
      className: spec.class ?? "",
      lang: spec.attributes?.["data-md-lang"],
    });
    iterator.next();
  }
  return result;
}

const TABLE_DOC = "| a | b |\n| --- | --- |\n| 1 | 2 |\n\n正文";
const CODE_DOC = '```ts\nconst a = 1;\n```\n\n正文';

describe("buildBlockDecorations", () => {
  it("光标在表格外时整表替换为部件", () => {
    const all = digests(TABLE_DOC, TABLE_DOC.length);
    const widget = all.find((item) => item.widget);
    expect(widget?.slice).toBe("| a | b |\n| --- | --- |\n| 1 | 2 |");
  });

  it("光标进入表格时仍保持表格部件", () => {
    expect(digests(TABLE_DOC, 2).some((item) => item.widget)).toBe(true);
  });

  it("光标在代码块外时隐藏围栏行并标注语言", () => {
    const all = digests(CODE_DOC, CODE_DOC.length);
    const hidden = all.filter((item) => !item.widget && !item.line && item.className === "");
    expect(hidden.map((item) => item.slice)).toEqual(["```ts", "```"]);
    const first = all.find((item) => item.className.includes("cm-md-codeline-first"));
    expect(first?.lang).toBe("ts");
  });

  it("光标进入代码块内容时仍隐藏围栏行", () => {
    const all = digests(CODE_DOC, 8);
    const hidden = all.filter((item) => !item.widget && !item.line && item.className === "");
    expect(hidden.map((item) => item.slice)).toEqual(["```ts", "```"]);
    expect(all.filter((item) => item.className.includes("cm-md-codeline"))).toHaveLength(1);
  });

  it("未闭合的代码块保留围栏，正在输入的开栏行不会消失", () => {
    const doc = "```ts\nconst a = 1;";
    const all = digests(doc, doc.length);
    expect(all.some((item) => !item.widget && !item.line && item.className === "")).toBe(false);
  });

  it("公式块在光标外渲染为部件，光标进入后显示源码并在下方预览", () => {
    const doc = "$$\nx^2\n$$\n\n正文";
    expect(digests(doc, doc.length).find((item) => item.widget)?.slice).toBe("$$\nx^2\n$$");
    const editing = digests(doc, 3);
    expect(editing.filter((item) => item.className === "cm-md-math-line")).toHaveLength(3);
    expect(editing.some((item) => item.widget && item.slice === "")).toBe(true);
  });

  it("空代码块不隐藏围栏，否则无法再定位", () => {
    const doc = "```\n```\n\n正文";
    const all = digests(doc, doc.length);
    expect(all.every((item) => item.className !== "" || item.widget)).toBe(true);
  });
});
