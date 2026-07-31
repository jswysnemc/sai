import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { GatewayBrandIcon } from "./gateway-brand-icon";

/**
 * 渲染指定网关的品牌图标。
 *
 * @param gatewayId 网关标识
 * @returns SVG 静态标记
 */
function renderIcon(gatewayId: string): string {
  return renderToStaticMarkup(<GatewayBrandIcon gatewayId={gatewayId} />);
}

describe("GatewayBrandIcon", () => {
  it("为 QQ 和微信渲染可区分的 SVG 品牌标记", () => {
    const qq = renderIcon("qq");
    const weixin = renderIcon("weixin");

    expect(qq).toContain('data-gateway-brand="qq"');
    expect(weixin).toContain('data-gateway-brand="weixin"');
    expect(qq).not.toBe(weixin);
  });

  it("未知网关不伪装成已有品牌", () => {
    expect(renderIcon("custom")).toBe("");
  });
});
