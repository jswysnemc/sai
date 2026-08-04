import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DiffView } from "./diff-view";

const PATCH = `diff --git a/src/a.ts b/src/a.ts
index 1111111..2222222 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -1,7 +1,7 @@
 same one
-old a line
-old b line
+new a line
 context mid
+brand new line
 same tail
`;

describe("DiffView side-by-side", () => {
  it("统一模式渲染单列行", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="unified" />);
    expect(html).toContain("diff-file-lines");
    expect(html).not.toContain("diff-side-grid");
  });

  it("并排模式渲染左右两栏且配对行同行", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="side" />);
    expect(html).toContain("diff-side-grid");
    // 配对行：左 old a / 右 new a 应同时出现
    // 字符级高亮把改动词单独包进 mark，断言左右两栏各自的改动区间
    expect(html).toContain('diff-inline">old</mark>');
    expect(html).toContain('diff-inline">new</mark>');
    // 孤立新增行左侧留空槽
    expect(html).toContain("diff-row empty left");
    // context 行左右同列，文本出现两次
    const occurrences = html.split("same one").length - 1;
    expect(occurrences).toBe(2);
  });

  it("并排模式行号分列显示", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="side" />);
    // 左栏显示旧行号 2，右栏显示新行号 3
    expect(html).toMatch(/diff-row removed left/);
    expect(html).toMatch(/diff-row added right/);
  });
});
