import { describe, expect, it } from "vitest";
import { isMarkdownFile } from "./markdown-file-preview";

describe("isMarkdownFile", () => {
  it("识别常用 Markdown 扩展名", () => {
    expect(isMarkdownFile("README.md")).toBe(true);
    expect(isMarkdownFile("docs/guide.MARKDOWN")).toBe(true);
  });

  it("拒绝非 Markdown 文件", () => {
    expect(isMarkdownFile("notes.txt")).toBe(false);
  });
});
