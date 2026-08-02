import { renderWithProviders } from "../../../shared/testing/render-with-providers";
import { describe, expect, it } from "vitest";
import { UserMessageBubble } from "./user-message-bubble";

describe("UserMessageBubble", () => {
  it("提供编辑入口时才渲染编辑按钮", () => {
    const withEdit = renderWithProviders(
      <UserMessageBubble content="原文" onEditResend={() => {}} />
    );
    const withoutEdit = renderWithProviders(<UserMessageBubble content="原文" />);

    expect(withEdit).toContain('aria-label="编辑并重新发送"');
    expect(withoutEdit).not.toContain("编辑并重新发送");
  });

  it("用户气泡不再承载重试入口，重试挂在助手回复上", () => {
    const html = renderWithProviders(
      <UserMessageBubble content="原文" onEditResend={() => {}} onContinueFrom={() => {}} />
    );

    expect(html).not.toContain("重试本轮");
  });
});
