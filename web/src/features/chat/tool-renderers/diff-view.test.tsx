import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DiffView } from "./diff-view";

const PATCH = `diff --git a/src/a.ts b/src/a.ts
index 1111111..2222222 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -1,9 +1,9 @@
 ctx one
 ctx two
 ctx three
 ctx four
-old a line
-old b line
+new a line
 ctx mid one
 ctx mid two
 ctx mid three
 ctx mid four
+brand new line
 same tail
`;

describe("DiffView unified", () => {
  it("统一模式渲染单列行", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="unified" />);
    expect(html).toContain("diff-file-lines");
    expect(html).not.toContain("diff-idea");
  });
});

describe("DiffView idea side-by-side", () => {
  it("渲染 IDEA 式查看器与变更块导航", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="side" />);
    expect(html).toContain("diff-idea");
    expect(html).toContain("diff-idea-toolbar");
    // 两处变更块
    expect(html).toContain("2 处变更");
    expect(html).toContain("1 / 2");
  });

  it("变更块带中间连接带且按增删着色", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="side" />);
    // 第一块删除+新增混合，第二块纯新增
    expect(html).toContain("diff-tone-mixed");
    expect(html).toContain("diff-tone-added");
    expect(html).toContain("diff-idea-band");
  });

  it("短上下文段直接铺开不折叠", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="side" />);
    // 各上下文段均不超过两倍边距，不产生折条
    expect(html).not.toContain("diff-idea-fold");
  });

  it("长上下文段折叠为折条并可展开", () => {
    const longContext = Array.from({ length: 12 }, (_, index) => `ctx ${index + 1}`).join("\n");
    const longPatch = `diff --git a/src/b.ts b/src/b.ts
--- a/src/b.ts
+++ b/src/b.ts
@@ -1,14 +1,14 @@
-old line
+new line
${longContext}
`;
    const html = renderToStaticMarkup(<DiffView source={longPatch} layout="side" />);
    // 13 行上下文超过两倍边距，中间 7 行折叠
    expect(html).toContain("diff-idea-fold");
    expect(html).toContain("展开 7 行未改动内容");
  });

  it("配对行左右同列且字符级高亮", () => {
    const html = renderToStaticMarkup(<DiffView source={PATCH} layout="side" />);
    expect(html).toContain('diff-inline">old</mark>');
    expect(html).toContain('diff-inline">new</mark>');
    // 左右行号分列
    expect(html).toMatch(/diff-row removed left/);
    expect(html).toMatch(/diff-row added right/);
  });
});
