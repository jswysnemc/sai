import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ToolStatusMark } from "./tool-icon";

describe("tool status mark", () => {
  it("参数拼接阶段只渲染动态图标", () => {
    const markup = renderToStaticMarkup(<ToolStatusMark state="preparing" />);

    expect(markup).toContain("tool-arguments-spinner");
    expect(markup).not.toContain("Preparing");
    expect(markup).not.toContain("准备中");
  });

  it("工具运行阶段不重复渲染状态标记", () => {
    expect(renderToStaticMarkup(<ToolStatusMark state="running" />)).toBe("");
  });
});
