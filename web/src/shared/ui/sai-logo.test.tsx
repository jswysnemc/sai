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
  it("renders the prompt glyph with a baseline cursor block", () => {
    const html = renderLogo();

    expect(html).toContain('aria-label="Sai"');
    // 双箭头提示符加 2x2 光标块，共 14 个实心格
    expect(html.match(/<rect/g)).toHaveLength(14);
    // 首格与右下角光标块的位置锚点
    expect(html).toContain('<rect x="3.75" y="7.25" width="3.5" height="3.5"></rect>');
    expect(html).toContain('<rect x="24.75" y="21.25" width="3.5" height="3.5"></rect>');
    expect(html).not.toContain("linearGradient");
  });
});
