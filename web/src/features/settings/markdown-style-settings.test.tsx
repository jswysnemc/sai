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

  it("内嵌效果预览，并按当前配置渲染示例", () => {
    const html = renderToStaticMarkup(
      <MarkdownStyleSettings
        preferences={DEFAULT_MARKDOWN_STYLE_PREFERENCES}
        onTableChange={vi.fn()}
        onCodeBlockChange={vi.fn()}
        onReset={vi.fn()}
      />
    );

    expect(html).toContain("效果预览");
    // 示例内容随配置一起渲染，而不是静态截图
    expect(html).toContain("deepseek-v4-pro");
  });

  it("预览跟随配置变化：关闭语言标签后不再出现语言名", () => {
    const withLabel = renderToStaticMarkup(
      <MarkdownStyleSettings
        preferences={DEFAULT_MARKDOWN_STYLE_PREFERENCES}
        onTableChange={vi.fn()}
        onCodeBlockChange={vi.fn()}
        onReset={vi.fn()}
      />
    );
    const withoutLabel = renderToStaticMarkup(
      <MarkdownStyleSettings
        preferences={{
          ...DEFAULT_MARKDOWN_STYLE_PREFERENCES,
          codeBlock: { ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock, showLanguageLabel: false }
        }}
        onTableChange={vi.fn()}
        onCodeBlockChange={vi.fn()}
        onReset={vi.fn()}
      />
    );

    expect(withLabel).not.toEqual(withoutLabel);
  });
});
