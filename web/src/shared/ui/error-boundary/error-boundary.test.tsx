import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./error-boundary";
import { ErrorFallback } from "./error-fallback";

/*
 * 注意：错误边界只在客户端渲染的 commit 阶段生效，
 * renderToStaticMarkup 遇到异常会直接向上抛出而不调用边界。
 * 因此这里分开验证两件事：状态派生逻辑本身，以及降级内容的渲染结果。
 */

describe("ErrorBoundary", () => {
  it("正常渲染时透传子节点", () => {
    const html = renderToStaticMarkup(
      <ErrorBoundary>
        <p>content</p>
      </ErrorBoundary>
    );
    expect(html).toContain("content");
  });

  it("把捕获到的异常派生为组件状态", () => {
    const error = new Error("boom");
    expect(ErrorBoundary.getDerivedStateFromError(error)).toEqual({ error });
  });

  it("捕获后记录组件栈便于排查", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const boundary = new ErrorBoundary({ children: null, label: "面板" });
    boundary.componentDidCatch(new Error("boom"), { componentStack: "at Panel" } as never);
    expect(consoleError).toHaveBeenCalledOnce();
    expect(String(consoleError.mock.calls[0][0])).toContain("面板");
    consoleError.mockRestore();
  });
});

describe("ErrorFallback", () => {
  it("展示错误信息与重试入口", () => {
    const html = renderToStaticMarkup(
      <ErrorFallback error={new Error("boom")} onRetry={() => undefined} />
    );
    expect(html).toContain("boom");
    expect(html).toContain("</button>");
  });

  it("支持自定义区域名称", () => {
    const html = renderToStaticMarkup(
      <ErrorFallback error={new Error("boom")} label="Git 面板" onRetry={() => undefined} />
    );
    expect(html).toContain("Git 面板");
  });
});
