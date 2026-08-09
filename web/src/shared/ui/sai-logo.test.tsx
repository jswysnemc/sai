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
  it("renders the Sai lettermark as grid-aligned rects", () => {
    const html = renderLogo();

    expect(html).toContain('aria-label="Sai"');
    // 半块字符网格展开后的矩形总数（█/▀/▄ 各计一个）
    expect(html.match(/<rect/g)).toHaveLength(42);
    // 位置锚点：S 左上起点与 i 的圆点
    expect(html).toContain('<rect x="3.25" y="11.5" width="1.5" height="1.5"></rect>');
    expect(html).toContain('<rect x="24.25" y="11.5" width="1.5" height="1.5"></rect>');
    expect(html).not.toContain("linearGradient");
  });
});
