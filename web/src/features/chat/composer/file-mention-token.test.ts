import { describe, expect, it } from "vitest";
import { findFileMentionTrigger, formatFileMention, parseFileMentions } from "./file-mention-token";

describe("file mention token", () => {
  it("parses mentions at text boundaries without converting email addresses", () => {
    expect(parseFileMentions("检查 @web/src/main.tsx 和 test@example.com")).toEqual([
      { type: "text", value: "检查 " },
      { type: "mention", path: "web/src/main.tsx", value: "@web/src/main.tsx" },
      { type: "text", value: " 和 test@example.com" }
    ]);
  });

  it("quotes paths containing whitespace and restores their original value", () => {
    const mention = formatFileMention("docs/design draft.md");
    expect(mention).toBe('@"docs/design draft.md"');
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
