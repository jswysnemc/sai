import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_PROMPT_TEMPLATES } from "./prompt-template-catalog";
import { PromptTemplateSettings } from "./prompt-template-settings";

describe("PromptTemplateSettings", () => {
  it("展示三类内部提示词及各自可用变量", () => {
    const html = renderToStaticMarkup(
      <PromptTemplateSettings
        templates={DEFAULT_PROMPT_TEMPLATES}
        onChange={vi.fn()}
      />
    );

    expect(html).toContain("Git 提交说明");
    expect(html).toContain("会话标题");
    expect(html).toContain("上下文压缩");
    expect(html).toContain("{{status}}");
    expect(html).toContain("{{assistant_preview}}");
    expect(html).toContain("{{history}}");
  });
});
