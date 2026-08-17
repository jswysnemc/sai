import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EditToolView } from "./edit-tool-view";

const UNIFIED = [
  "diff --git a/notes.txt b/notes.txt",
  "--- a/notes.txt",
  "+++ b/notes.txt",
  "@@ -1,3 +1,3 @@",
  " one",
  "-two",
  "+TWO",
  " three"
].join("\n");

describe("EditToolView", () => {
  it("优先渲染工具结果里带上下文的 unified diff，而不是参数拼出的红蓝行", () => {
    const html = renderToStaticMarkup(
      <EditToolView
        argumentsText={JSON.stringify({
          path: "notes.txt",
          old_string: "two",
          new_string: "TWO"
        })}
        output={JSON.stringify({ diff: UNIFIED })}
        headerPath="notes.txt"
      />
    );

    expect(html).toContain("one");
    expect(html).toContain("three");
    expect(html).toContain("diff-unified-row context");
    expect(html).toContain("diff-unified-row removed");
    expect(html).toContain("diff-unified-row added");
  });

  it("没有结果 diff 时仍用参数合成预览", () => {
    const html = renderToStaticMarkup(
      <EditToolView
        argumentsText={JSON.stringify({
          path: "notes.txt",
          old_string: "two",
          new_string: "TWO"
        })}
        output=""
        headerPath="notes.txt"
      />
    );

    expect(html).toContain("diff-unified-row removed");
    expect(html).toContain("diff-unified-row added");
    expect(html).not.toContain("diff-unified-row context");
  });
});
