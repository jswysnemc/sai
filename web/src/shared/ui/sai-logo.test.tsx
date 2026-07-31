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
  it("renders the shared SAI grid with a terminal cursor", () => {
    const html = renderLogo();

    expect(html).toContain('aria-label="Sai"');
    expect(html.match(/<rect/g)).toHaveLength(30);
    expect(html).toContain('<rect x="12.25" y="9.75" width="2.5" height="2.5"></rect>');
    expect(html).toContain('<rect x="27.25" y="17.25" width="2.5" height="2.5"></rect>');
    expect(html).not.toContain("linearGradient");
  });
});
