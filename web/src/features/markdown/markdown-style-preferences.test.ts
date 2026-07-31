import { describe, expect, it } from "vitest";
import {
  DEFAULT_MARKDOWN_STYLE_PREFERENCES,
  parseMarkdownStylePreferences
} from "./markdown-style-preferences";

describe("parseMarkdownStylePreferences", () => {
  it("缺少本地配置时保持当前 Markdown 视觉默认值", () => {
    expect(parseMarkdownStylePreferences(null)).toEqual(DEFAULT_MARKDOWN_STYLE_PREFERENCES);
  });

  it("旧配置缺少字段时补齐默认值并保留合法选项", () => {
    const preferences = parseMarkdownStylePreferences(JSON.stringify({
      table: {
        borderStyle: "grid",
        stripedRows: true
      },
      codeBlock: {
        lineNumbers: true,
        tabSize: "4"
      }
    }));

    expect(preferences.table).toEqual({
      ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.table,
      borderStyle: "grid",
      stripedRows: true
    });
    expect(preferences.codeBlock).toEqual({
      ...DEFAULT_MARKDOWN_STYLE_PREFERENCES.codeBlock,
      lineNumbers: true,
      tabSize: "4"
    });
  });

  it("损坏或越界配置回退到安全默认值", () => {
    expect(parseMarkdownStylePreferences("not-json")).toEqual(DEFAULT_MARKDOWN_STYLE_PREFERENCES);

    const preferences = parseMarkdownStylePreferences(JSON.stringify({
      table: {
        borderStyle: "double",
        density: "huge",
        fullWidth: "yes"
      },
      codeBlock: {
        fontSize: "32px",
        tabSize: 16,
        maxHeight: "12rem",
        showCopyButton: 1
      }
    }));

    expect(preferences).toEqual(DEFAULT_MARKDOWN_STYLE_PREFERENCES);
  });
});
