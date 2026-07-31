import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_MARKDOWN_STYLE_PREFERENCES } from "../markdown/markdown-style-preferences";
import { MarkdownStyleSettings } from "./markdown-style-settings";

describe("MarkdownStyleSettings", () => {
  it("展示表格与代码块的细粒度外观选项", () => {
    const html = renderToStaticMarkup(
      <MarkdownStyleSettings
        preferences={DEFAULT_MARKDOWN_STYLE_PREFERENCES}
        onTableChange={vi.fn()}
        onCodeBlockChange={vi.fn()}
        onReset={vi.fn()}
      />
    );

    expect(html).toContain("Markdown 渲染");
    expect(html).toContain("表格边框");
    expect(html).toContain("单元格密度");
    expect(html).toContain("斑马纹");
    expect(html).toContain("显示行号");
    expect(html).toContain("长行换行");
    expect(html).toContain("制表符宽度");
    expect(html).toContain("最大高度");
  });
});
