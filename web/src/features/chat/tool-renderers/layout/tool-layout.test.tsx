import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ToolLayout } from "./tool-layout";

describe("ToolLayout", () => {
  it("renders the kind label, texts and status in one summary row", () => {
    const html = renderToStaticMarkup(
      <ToolLayout
        kindLabel="已运行"
        primaryText="build.sh"
        secondaryText="$ cargo build"
        statusLabel="退出码 0"
      />
    );

    expect(html).toContain("已运行");
    expect(html).toContain("build.sh");
    expect(html).toContain("$ cargo build");
    expect(html).toContain("退出码 0");
  });

  it("marks the running state with the gradient text instead of a separate badge", () => {
    // 运行态若再占一个状态位，摘要行会被状态文字挤掉操作对象
    const html = renderToStaticMarkup(<ToolLayout kindLabel="运行中" isRunning />);

    expect(html).toContain("animated-gradient-text");
  });

  it("keeps the kind label static when it is not running", () => {
    const html = renderToStaticMarkup(<ToolLayout kindLabel="已运行" />);

    expect(html).not.toContain("animated-gradient-text");
  });

  it("colors the status as a failure only when asked", () => {
    const failed = renderToStaticMarkup(
      <ToolLayout kindLabel="已运行" statusLabel="出错" showFailureStatus />
    );
    const normal = renderToStaticMarkup(<ToolLayout kindLabel="已运行" statusLabel="完成" />);

    expect(failed).toContain("text-destructive");
    expect(normal).not.toContain("text-destructive");
  });

  it("exposes the expand state to assistive technology", () => {
    const html = renderToStaticMarkup(
      <ToolLayout kindLabel="已运行" expanded onToggle={() => undefined}>
        <p>详情</p>
      </ToolLayout>
    );

    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain('role="button"');
    expect(html).toContain("详情");
  });

  it("hides the content and the toggle affordance when it cannot expand", () => {
    // 不可折叠的卡片仍显示箭头会让用户以为点得开
    const html = renderToStaticMarkup(
      <ToolLayout kindLabel="已读取" canToggle={false}>
        <p>详情</p>
      </ToolLayout>
    );

    expect(html).not.toContain('role="button"');
    expect(html).not.toContain("详情");
  });

  it("renders the diff badge and hides it once expanded when requested", () => {
    const collapsed = renderToStaticMarkup(
      <ToolLayout kindLabel="已编辑" diffCount={{ added: 12, removed: 3 }} />
    );
    const expanded = renderToStaticMarkup(
      <ToolLayout
        kindLabel="已编辑"
        diffCount={{ added: 12, removed: 3 }}
        hideDiffCountWhenOpen
        expanded
        onToggle={() => undefined}
      />
    );

    expect(collapsed).toContain("+12");
    expect(collapsed).toContain("-3");
    expect(expanded).not.toContain("+12");
  });

  it("renders the source badge for delegated calls", () => {
    const html = renderToStaticMarkup(<ToolLayout kindLabel="已运行" sourceLabel="子智能体" />);

    expect(html).toContain("子智能体");
  });
});
