import { describe, expect, it } from "vitest";
import { findFileMentionTrigger, formatFileMention, parseFileMentions } from "./file-mention-token";

describe("file mention token", () => {
  it("only special-cases successful file-reference picks", () => {
    const mention = formatFileMention("web/src/main.tsx");
    expect(mention).toBe('<file-reference path="web/src/main.tsx"></file-reference>');
    expect(parseFileMentions(`检查 ${mention} 和 test@example.com 以及 @handwritten/path`)).toEqual([
      { type: "text", value: "检查 " },
      { type: "mention", path: "web/src/main.tsx", value: mention },
      { type: "text", value: " 和 test@example.com 以及 @handwritten/path" }
    ]);
  });

  it("quotes and restores paths containing whitespace", () => {
    const mention = formatFileMention("docs/design draft.md");
    expect(mention).toBe('<file-reference path="docs/design draft.md"></file-reference>');
    expect(parseFileMentions(mention)).toEqual([
      { type: "mention", path: "docs/design draft.md", value: mention }
    ]);
  });

  it("keeps plain text unchanged", () => {
    expect(parseFileMentions("普通输入")).toEqual([{ type: "text", value: "普通输入" }]);
  });

  it("detects an at-sign immediately before the contenteditable caret", () => {
    expect(findFileMentionTrigger("fix @", 5, "@")).toEqual({ start: 4, end: 5 });
    expect(findFileMentionTrigger("fix @file", 9, "@")).toBeNull();
    expect(findFileMentionTrigger("fix @", 4, "@")).toBeNull();
    expect(findFileMentionTrigger("fix @", 5, null)).toBeNull();
  });
});
