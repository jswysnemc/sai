import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { activeHeadingIndex, extractOutline, plainHeadingText } from "./outline-model";

/**
 * 构建带 Markdown 语法的编辑器状态。
 *
 * @param doc 文档内容
 * @returns 编辑器状态
 */
function stateOf(doc: string) {
  return EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage })] });
}

describe("extractOutline", () => {
  it("按文档顺序提取各级标题", () => {
    const doc = "# 一\n\n正文\n\n## 二\n\n### 三";
    expect(extractOutline(stateOf(doc)).map((item) => [item.level, item.text])).toEqual([
      [1, "一"],
      [2, "二"],
      [3, "三"],
    ]);
  });

  it("忽略代码块里的井号行", () => {
    const doc = "# 标题\n\n```sh\n# 注释\n```";
    expect(extractOutline(stateOf(doc)).map((item) => item.text)).toEqual(["标题"]);
  });

  it("支持 Setext 写法且只取首行文字", () => {
    const doc = "主标题\n===\n\n副标题\n---";
    expect(extractOutline(stateOf(doc)).map((item) => [item.level, item.text])).toEqual([
      [1, "主标题"],
      [2, "副标题"],
    ]);
  });

  it("记录标题起始偏移", () => {
    const doc = "正文\n\n## 二";
    expect(extractOutline(stateOf(doc))[0].from).toBe(4);
  });
});

describe("plainHeadingText", () => {
  it("去掉行内标记、链接地址与闭合井号", () => {
    expect(plainHeadingText("## **重点** 与 [文档](https://a.b) `code` ##")).toBe("重点 与 文档 code");
  });
});

describe("activeHeadingIndex", () => {
  const headings = [
    { level: 1, text: "a", from: 0 },
    { level: 2, text: "b", from: 10 },
    { level: 2, text: "c", from: 20 },
  ];

  it("取位置之前最近的标题", () => {
    expect(activeHeadingIndex(headings, 15)).toBe(1);
    expect(activeHeadingIndex(headings, 20)).toBe(2);
  });

  it("位置在首个标题之前时为 -1", () => {
    expect(activeHeadingIndex([{ level: 1, text: "a", from: 5 }], 2)).toBe(-1);
  });
});
