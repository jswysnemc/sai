import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { splitMarkdownStream, type MarkdownStreamState } from "./markdown-stream-blocks";
import { MarkdownContent, EMPTY_INLINE_ATOMS } from "./markdown-content";
import { DEFAULT_MARKDOWN_STYLE_PREFERENCES as style } from "../markdown/markdown-style-preferences";

const parser = unified().use(remarkParse).use(remarkGfm).use(remarkMath);
const fixtures = [
  "Intro\n\nBefore\n\n   - item\n       continuation\n\n       code?\n\nAfter\n\nLast",
  "# Heading\n\nparagraph\n\n| A | B |\n| - | - |\n| x | y |\n\nnext\n\nlast",
  "Intro\n\n- first\n\n  continuation\n  - nested\n\n- second\n\nAfter\n\nlast",
  "> quote\n>\n> continued\n\n> another\n\nOutside\n\n---\n\nend",
  "Before\n\n```ts\nconst x = 1;\n\n# inside fence\n```\n\nAfter\n\nlast",
  "Before\n\n$$\nx^2\n\n+ y^2\n$$\n\n$x+1$\n\nlast",
  "Paragraph\n--------\n\n    code\n\n    continuation\n\nAfter\n\nlast",
  "[link][ref] and note[^one]\n\nMiddle\n\nMore\n\n[ref]: https://example.com\n\n[^one]: footnote",
  "<svg viewBox=\"0 0 1 1\">\n<path d=\"M0 0\"/>\n</svg>\n\nAfter\n\nlast",
  "Before\n\n> [!NOTE]\n> body\n\n`sai-atom-0`\n\nlast",
];

/** 【正文渲染】【语义比较】参数为文本，返回剔除源位置后的语法节点。 */
function nodes(source: string): unknown[] {
  return JSON.parse(JSON.stringify(parser.parse(source).children, (key, value) => key === "position" ? undefined : value));
}

/** 【正文渲染】【组件比较】参数为文本，返回统一容器内的实际渲染 HTML。 */
function html(source: string): string {
  return renderToStaticMarkup(<MarkdownContent source={source} inlineAtoms={EMPTY_INLINE_ATOMS} style={style} streaming={false} />)
    .replace(/^<div[^>]*>/, "").replace(/<\/div>$/, "");
}

describe("Markdown streaming blocks", () => {
  it.each(fixtures)("逐字符输入保持完整解析语义：%s", (source) => {
    let state: MarkdownStreamState | undefined;
    for (let length = 1; length <= source.length; length += 1) {
      const prefix = source.slice(0, length);
      state = splitMarkdownStream(prefix, state);
      expect(state.blocks.map((block) => block.source).join("")).toBe(prefix);
      expect(state.blocks.flatMap((block) => nodes(block.source)), `prefix length=${length}`).toEqual(nodes(prefix));
    }
    expect(state!.blocks.map((block) => html(block.source)).join("\n")).toBe(html(source));
  });

  it("新尾部复用完成块，非追加编辑重建分块", () => {
    const first = splitMarkdownStream("# First\n\nStable $x^2$\n\nTail\n\nLast");
    const next = splitMarkdownStream(`${first.source}!`, first);
    expect(next.blocks[0]).toBe(first.blocks[0]);
    expect(next.blocks[0].offset).toBe(0);
    expect(next.tailOffset).toBeGreaterThan(0);
    const edited = splitMarkdownStream("different", next);
    expect(edited.blocks).toEqual([{ offset: 0, source: "different" }]);
  });
});
