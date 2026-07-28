import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ChatSessionHeader } from "./chat-session-header";

/**
 * 渲染固定项目上下文的会话头部。
 *
 * @param branch 可选 Git 分支
 * @returns 会话头部静态标记
 */
function renderHeader(branch?: string): string {
  return renderToStaticMarkup(
    <ChatSessionHeader
      title="问候"
      workspace={{ name: "sai", path: "/home/snemc/workspace/sai" }}
      branch={branch}
    />
  );
}

describe("ChatSessionHeader", () => {
  it("将会话标题、项目路径和分支组成两级信息", () => {
    const html = renderHeader("main");

    expect(html).toContain("<h1");
    expect(html).toContain("问候");
    expect(html).toContain("/home/snemc/workspace/sai");
    expect(html).toContain("main");
    expect(html).toContain('aria-label="项目上下文"');
  });

  it("非 Git 工作区不渲染空分支项", () => {
    const html = renderHeader();

    expect(html).toContain("chat-header-workspace");
    expect(html).not.toContain("chat-header-branch");
  });

  it("项目上下文加载前保持标题区域稳定", () => {
    const html = renderToStaticMarkup(<ChatSessionHeader title="选择会话" />);

    expect(html).toContain("选择会话");
    expect(html).not.toContain("chat-header-context");
  });
});
