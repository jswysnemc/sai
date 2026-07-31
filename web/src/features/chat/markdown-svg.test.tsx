import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MarkdownRenderer } from "./markdown-renderer";
import { isSvgMarkup, remarkSvgBlocks, toSvgDataUrl } from "./markdown-svg";

describe("Markdown SVG", () => {
  it("accepts standalone SVG and encodes it as an image URL", () => {
    const source = '<svg viewBox="0 0 10 10"><circle cx="5" cy="5" r="4" /></svg>';

    expect(isSvgMarkup(source)).toBe(true);
    expect(toSvgDataUrl(source)).toContain("data:image/svg+xml;charset=utf-8,");
    expect(toSvgDataUrl("<div>not svg</div>")).toBeNull();
  });

  it("converts raw Markdown SVG HTML blocks into svg code nodes", () => {
    const tree = {
      type: "root",
      children: [{ type: "html", value: '<svg viewBox="0 0 1 1"></svg>' }]
    };

    remarkSvgBlocks()(tree);

    expect(tree.children[0]).toEqual({
      type: "code",
      lang: "svg",
      value: '<svg viewBox="0 0 1 1"></svg>'
    });
  });

  it("renders fenced and raw SVG blocks as scalable images", () => {
    const fenced = renderToStaticMarkup(
      <MarkdownRenderer source={'```svg\n<svg viewBox="0 0 8 8"><path d="M0 0h8v8z" /></svg>\n```'} />
    );
    const raw = renderToStaticMarkup(
      <MarkdownRenderer source={'<svg viewBox="0 0 8 8"><path d="M0 0h8v8z" /></svg>'} />
    );

    expect(fenced).toContain("markdown-svg-preview-image");
    expect(raw).toContain("markdown-svg-preview-image");
    expect(raw).not.toContain("dangerouslySetInnerHTML");
  });
});
