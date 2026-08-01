import { describe, expect, it } from "vitest";
import { buildAppearancePreviewSample } from "./appearance-preview-sample";

describe("buildAppearancePreviewSample", () => {
  it("同时包含表格与代码块，覆盖两组可调项", () => {
    const sample = buildAppearancePreviewSample("zh-CN");

    // 表格需要表头分隔行与多行数据，才能体现边框、密度与斑马纹
    expect(sample).toContain("| --- | --- | --- |");
    expect(sample.split("\n").filter((line) => line.startsWith("| ")).length).toBeGreaterThan(3);
    // 代码块需要带语言标识，才能体现语言标签开关
    expect(sample).toContain("```typescript");
  });

  it("代码示例含长行与缩进，便于观察折行和制表符宽度", () => {
    const sample = buildAppearancePreviewSample("zh-CN");
    const codeLines = sample.slice(sample.indexOf("```typescript")).split("\n");

    expect(codeLines.some((line) => line.length > 80)).toBe(true);
    expect(codeLines.some((line) => line.startsWith("  "))).toBe(true);
  });

  it("表头随界面语言切换", () => {
    expect(buildAppearancePreviewSample("zh-CN")).toContain("模型");
    expect(buildAppearancePreviewSample("en-US")).toContain("Model");
  });
});
