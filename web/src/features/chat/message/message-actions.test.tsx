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

  it("编辑重发排在开分支之前，二者都基于分支能力", () => {
    const html = render({ onContinueFrom: () => {}, onEdit: () => {} });

    expect(html).toContain('aria-label="编辑并重新发送"');
    expect(html).toContain('aria-label="从这里继续"');
    expect(html.indexOf("编辑并重新发送")).toBeLessThan(html.indexOf("从这里继续"));
  });

  it("未提供回调时不渲染对应按钮", () => {
    const html = render({});

    expect(html).not.toContain("从这里继续");
    expect(html).not.toContain("编辑并重新发送");
    expect(html).not.toContain("重试本轮");
    expect(html).toContain('aria-label="复制消息原文"');
  });

  it("动作进行中时禁用分支相关按钮", () => {
    const html = render({ onContinueFrom: () => {}, onEdit: () => {}, busy: true });

    // 编辑与开分支都会改动活动叶子，须一并跟随 busy 禁用
    expect(html.match(/disabled/gu)?.length).toBe(2);
  });
});
