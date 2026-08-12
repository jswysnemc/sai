import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ModelThinkingSelector } from "./model-thinking-selector";

const currentChoice = { providerId: "openai", providerName: "OpenAI", model: "big-pickle" };
const pendingChoice = { providerId: "deepseek", providerName: "DeepSeek", model: "deepseek-v4-flash" };

/**
 * 以静态标记渲染统一模型选择器。
 *
 * @param pending 待生效模型；null 表示无暂存
 * @param disabled 是否禁用触发器
 * @returns 渲染后的 HTML 字符串
 */
function renderSelector(pending: typeof pendingChoice | null, disabled = false): string {
  return renderToStaticMarkup(
    <ModelThinkingSelector
      choices={[currentChoice, pendingChoice]}
      selection={currentChoice}
      pendingSelection={pending}
      thinkingLevel="auto"
      loading={false}
      disabled={disabled}
      onModelSelect={() => undefined}
      onThinkingLevelChange={() => undefined}
    />
  );
}

describe("ModelThinkingSelector 待生效标识", () => {
  it("存在待生效模型时展示下轮生效徽标", () => {
    const markup = renderSelector(pendingChoice);

    expect(markup).toContain("model-thinking-pending");
    expect(markup).toContain("deepseek-v4-flash 下轮生效");
  });

  it("无待生效模型时不渲染徽标", () => {
    expect(renderSelector(null)).not.toContain("model-thinking-pending");
  });

  it("运行中保持触发器可用", () => {
    // 需求：聊天中不再禁用模型选择器；disabled 传 false 时按钮不带 disabled 属性
    expect(renderSelector(null, false)).not.toContain("disabled");
  });
});
