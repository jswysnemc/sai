import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MessageActions } from "./message-actions";

/**
 * 渲染消息操作行并返回静态标记。
 *
 * @param props 需要覆盖的操作回调
 * @returns 操作行静态标记
 */
function render(props: Partial<Parameters<typeof MessageActions>[0]>): string {
  return renderToStaticMarkup(<MessageActions text="回复正文" {...props} />);
}

describe("MessageActions", () => {
  it("提供从当前轮次开分支的入口", () => {
    const html = render({ onContinueFrom: () => {} });

    expect(html).toContain('aria-label="从这里继续"');
    expect(html).toContain("形成新分支");
  });

  it("把复制为新会话与分支区分开，避免术语混淆", () => {
    const html = render({ onContinueFrom: () => {}, onFork: () => {} });

    expect(html).toContain('aria-label="从这里继续"');
    expect(html).toContain('aria-label="复制为新会话"');
    // 分支动作排在复制会话之前：前者是常用操作
    expect(html.indexOf("从这里继续")).toBeLessThan(html.indexOf("复制为新会话"));
  });

  it("未提供回调时不渲染对应按钮", () => {
    const html = render({});

    expect(html).not.toContain("从这里继续");
    expect(html).not.toContain("复制为新会话");
    expect(html).not.toContain("重试本轮");
    expect(html).toContain('aria-label="复制消息原文"');
  });

  it("动作进行中时禁用分支按钮", () => {
    const html = render({ onContinueFrom: () => {}, onFork: () => {}, busy: true });

    // 两个分支相关按钮都要跟随 busy 禁用，避免并发切换活动叶子
    expect(html.match(/disabled/gu)?.length).toBe(2);
  });
});
