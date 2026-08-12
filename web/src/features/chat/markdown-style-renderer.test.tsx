import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DEFAULT_MARKDOWN_STYLE_PREFERENCES } from "../markdown/markdown-style-preferences";
import { MarkdownRenderer } from "./markdown-renderer";

describe("MarkdownRenderer style preferences", () => {
  it("将项目内文件路径渲染为轻量可点击引用", () => {
    const html = renderToStaticMarkup(<MarkdownRenderer source="打开 `login-page/index.html` 查看页面。" />);
    expect(html).toContain("inline-file-reference");
    expect(html).toContain("/material-icons/html.svg");
    expect(html).toContain("login-page/index.html");
  });

  it("密文件 pill 与中文说明同段渲染，保留说明文本节点", () => {
    const source =
      "`docs/00-a.md` `docs/01-b.md` —— 研究文档集 / 工具调研 / `docs/02-c.md` 综合与路线图";
    const html = renderToStaticMarkup(<MarkdownRenderer source={source} />);
    expect(html).toContain("inline-file-reference");
    expect(html).toContain("—— 研究文档集 / 工具调研 /");
    expect(html).toContain("综合与路线图");
    expect((html.match(/inline-file-reference/g) ?? []).length).toBe(3);
  });

  it("将表格和代码块配置映射到实际渲染结构", () => {
    const stylePreferences = {
      preset: "compact" as const,
      table: {
        ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.table,
        borderStyle: "grid" as const,
        density: "compact" as const,
        fullWidth: false,
        stripedRows: true
      },
      codeBlock: {
        ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock,
        lineNumbers: true,
        wrapLongLines: true,
        showLanguageLabel: false,
        showCopyButton: false,
        showBorder: true,
        tabSize: "4" as const,
        maxHeight: "medium" as const
      }
    };
    const html = renderToStaticMarkup(
      <MarkdownRenderer
        source={"| A | B |\n| - | - |\n| 1 | 2 |\n\n```ts\nconst value = 1;\n```"}
        stylePreferences={stylePreferences}
      />
    );

    expect(html).toContain('data-md-preset="compact"');
    expect(html).toContain('data-table-border="grid"');
    expect(html).toContain('data-table-density="compact"');
    expect(html).toContain('data-table-width="content"');
    expect(html).toContain('data-code-wrap="true"');
    expect(html).toContain('data-code-border="true"');
    expect(html).toContain('data-code-tab-size="4"');
    expect(html).toContain('data-code-max-height="medium"');
    expect(html).toContain("syntax-line-number");
    expect(html).not.toContain("markdown-code-head");
  });
});
