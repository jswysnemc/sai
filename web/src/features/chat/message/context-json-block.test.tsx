import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MarkdownContent, EMPTY_INLINE_ATOMS } from "../markdown-content";
import { DEFAULT_MARKDOWN_STYLE_PREFERENCES } from "../../markdown/markdown-style-preferences";

describe("context tool JSON", () => {
  it("keeps tool headings and descriptions while initially hiding JSON fields", () => {
    const html = renderToStaticMarkup(<MarkdownContent
      source={'### read_file\n\nRead a file.\n\n```json\n{"properties":{"path":{"type":"string"}}}\n```'}
      inlineAtoms={EMPTY_INLINE_ATOMS} style={DEFAULT_MARKDOWN_STYLE_PREFERENCES} streaming={false} collapseJson
    />);
    expect(html).toContain("read_file");
    expect(html).toContain("Read a file.");
    expect(html).toContain('aria-expanded="false"');
    expect(html).not.toContain("properties");
    expect(html).not.toContain("syntax-highlight");
  });

  it("keeps ordinary chat JSON expanded", () => {
    const html = renderToStaticMarkup(<MarkdownContent source={'```json\n{"properties":{}}\n```'} inlineAtoms={EMPTY_INLINE_ATOMS} style={DEFAULT_MARKDOWN_STYLE_PREFERENCES} streaming={false} />);
    expect(html).toContain("properties");
    expect(html).not.toContain('aria-expanded="false"');
  });
});
