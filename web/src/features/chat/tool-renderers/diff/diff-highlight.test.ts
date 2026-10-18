import { describe, expect, it } from "vitest";
import { rememberSyntectHtml } from "../../syntax-highlighter";
import { highlightDiffLines, markChangedText } from "./diff-highlight";
import { parseDiff } from "./diff-parser";

describe("diff syntax highlighting", () => {
  it("字符差异不会覆盖修改行的语法着色", () => {
    const file = parseDiff("--- a/app.ts\n+++ b/app.ts\n@@ -1 +1 @@\n-const total = 1;\n+const total = 2;")[0];
    rememberSyntectHtml("const total = 2;", "ts", '<span class="hljs-keyword">const</span> total = <span class="hljs-number">2</span>;');
    const highlighted = highlightDiffLines(file.lines, "ts");
    const added = file.lines.find((line) => line.kind === "added")!;
    expect(highlighted.get(added)?.new).toContain('class="hljs-keyword"');
    expect(highlighted.get(added)?.new).toContain('class="diff-inline"');
    expect(highlighted.get(added)?.new).toContain(">2</mark>");
  });

  it("连续行保留跨行注释的语法状态", () => {
    const file = parseDiff("--- a/app.ts\n+++ b/app.ts\n@@ -1,3 +1,3 @@\n /* explanation\n- old text\n+ new text\n  */")[0];
    const added = file.lines.find((line) => line.kind === "added")!;
    const source = file.lines.filter((line) => line.kind !== "hunk" && line.kind !== "removed" && line.kind !== "no-newline").map((line) => line.text).join("\n");
    rememberSyntectHtml(source, "ts", `<span class="hljs-comment">${source}</span>`);
    expect(highlightDiffLines(file.lines, "ts").get(added)?.new).toContain('class="hljs-comment"');
  });

  it("字符区间跨越语法标签和 HTML 实体时仍保留安全转义", () => {
    const html = '<span class="hljs-string">&quot;a&lt;b&quot;</span>';
    const marked = markChangedText(html, [
      { text: '"a', changed: false }, { text: "<b", changed: true }, { text: '"', changed: false },
    ]);
    expect(marked).toContain('<mark class="diff-inline">&lt;</mark>');
    expect(marked).toContain('<mark class="diff-inline">b</mark>');
    expect(marked).not.toContain("<b");
    expect(marked).toContain('class="hljs-string"');
  });

  it("补丁中的 HTML 代码只作为文本展示", () => {
    const file = parseDiff('*** Add File: page.html\n+<script>alert("text")</script>')[0];
    const html = highlightDiffLines(file.lines, "html").get(file.lines[0])?.new;
    expect(html).toContain("&lt;");
    expect(html).not.toContain("<script>");
  });
});
