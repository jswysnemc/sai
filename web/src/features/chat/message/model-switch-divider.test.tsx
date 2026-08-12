import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ModelSwitchDivider } from "./model-switch-divider";

describe("ModelSwitchDivider", () => {
  it("渲染带前后模型的分割线文案", () => {
    const markup = renderToStaticMarkup(
      <ModelSwitchDivider marker={{ from: "big-pickle", to: "deepseek-v4-flash" }} />
    );

    expect(markup).toContain("model-switch-divider");
    expect(markup).toContain("模型已切换：big-pickle → deepseek-v4-flash");
  });

  it("以分隔符角色暴露给辅助技术", () => {
    const markup = renderToStaticMarkup(
      <ModelSwitchDivider marker={{ from: "a", to: "b" }} />
    );

    expect(markup).toContain('role="separator"');
  });
});
