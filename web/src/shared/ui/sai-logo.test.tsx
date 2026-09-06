import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SaiLogo } from "./sai-logo";

/**
 * 渲染 Sai 标志的静态标记。
 *
 * @returns 可用于结构断言的 SVG 字符串
 */
function renderLogo(): string {
  return renderToStaticMarkup(<SaiLogo size={20} />);
}

describe("SaiLogo", () => {
  it("keeps the app icon accessible and square", () => {
    const html = renderLogo();

    expect(html).toContain('aria-label="Sai"');
    expect(html).toContain('width="20" height="20"');
    expect(renderToStaticMarkup(<SaiLogo size={48} trim />)).toContain('width="48" height="24"');
  });
});
