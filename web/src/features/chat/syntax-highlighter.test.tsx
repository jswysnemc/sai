import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { splitHighlightedLines, SyntaxHighlighter } from "./syntax-highlighter";

describe("SyntaxHighlighter", () => {
  it("启用行号时按源码行生成连续编号", () => {
    const html = renderToStaticMarkup(
      <SyntaxHighlighter language="typescript" source={"const first = 1;\nconst second = 2;"} showLineNumbers />
    );

    expect(html).toContain("syntax-line-number");
    expect(html).toContain(">1</span>");
    expect(html).toContain(">2</span>");
  });

  it("拆分跨行高亮标签时保持每一行标签闭合", () => {
    expect(splitHighlightedLines('<span class="hljs-comment">first\nsecond</span>')).toEqual([
      '<span class="hljs-comment">first</span>',
      '<span class="hljs-comment">second</span>'
    ]);
  });
});
