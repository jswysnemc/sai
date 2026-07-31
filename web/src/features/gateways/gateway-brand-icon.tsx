import "./gateway-brand-icon.css";

type GatewayBrandIconProps = {
  gatewayId: string;
  size?: number;
  className?: string;
};

type GatewayBrand = "qq" | "weixin";

/**
 * 渲染 QQ 或微信网关的紧凑 SVG 品牌图标。
 *
 * @param props 网关标识、图标尺寸和附加类名
 * @returns 对应品牌的 SVG；未知网关返回 null
 */
export function GatewayBrandIcon({ gatewayId, size = 20, className }: GatewayBrandIconProps) {
  const brand = resolveGatewayBrand(gatewayId);
  if (!brand) return null;
  const classes = ["gateway-brand-icon", `gateway-brand-icon--${brand}`, className]
    .filter(Boolean)
    .join(" ");

  return (
    <svg
      className={classes}
      data-gateway-brand={brand}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      focusable="false"
    >
      {brand === "qq" ? <QqGlyph /> : <WeixinGlyph />}
    </svg>
  );
}

/**
 * 将后端网关标识归一化为可渲染的品牌类型。
 *
 * @param gatewayId 后端返回的网关标识
 * @returns 品牌类型；不支持的标识返回 null
 */
function resolveGatewayBrand(gatewayId: string): GatewayBrand | null {
  const normalized = gatewayId.trim().toLowerCase();
  if (normalized === "qq") return "qq";
  if (normalized === "weixin" || normalized === "wechat") return "weixin";
  return null;
}

/**
 * 渲染简化企鹅轮廓，保持小尺寸下仍可识别 QQ。
 *
 * @returns QQ 图形节点
 */
function QqGlyph() {
  return (
    <g stroke="currentColor" strokeWidth="1.55" strokeLinecap="round" strokeLinejoin="round">
      <path d="M7.25 10.2C7.25 5.55 9.05 2.8 12 2.8s4.75 2.75 4.75 7.4c0 4.96-1.92 8.75-4.75 8.75S7.25 15.16 7.25 10.2Z" />
      <path d="M7.45 11.55c-1.3 1.42-2.14 3.12-2.42 4.78l2.67-1.07M16.55 11.55c1.3 1.42 2.14 3.12 2.42 4.78l-2.67-1.07" />
      <path d="M9.1 19.1 7.65 21.2M14.9 19.1l1.45 2.1M8.3 13.1h7.4" />
      <circle cx="10.15" cy="8.25" r="0.72" fill="currentColor" stroke="none" />
      <circle cx="13.85" cy="8.25" r="0.72" fill="currentColor" stroke="none" />
      <path d="m10.65 10.25 1.35.9 1.35-.9" />
    </g>
  );
}

/**
 * 渲染双气泡轮廓，保持小尺寸下仍可识别微信。
 *
 * @returns 微信图形节点
 */
function WeixinGlyph() {
  return (
    <g stroke="currentColor" strokeWidth="1.55" strokeLinecap="round" strokeLinejoin="round">
      <path d="M13.75 6.05A6.55 6.55 0 0 0 2.8 10.8c0 1.33.55 2.56 1.48 3.54l-.72 2.45 2.63-1.17c.77.34 1.63.53 2.53.53.42 0 .82-.04 1.21-.11" />
      <path d="M21.2 14.15c0-2.92-2.61-5.28-5.83-5.28s-5.82 2.36-5.82 5.28 2.6 5.28 5.82 5.28c.78 0 1.53-.14 2.2-.39l2.26 1-.59-2.07c1.21-.96 1.96-2.31 1.96-3.82Z" />
      <circle cx="6.75" cy="9.85" r="0.68" fill="currentColor" stroke="none" />
      <circle cx="10.75" cy="9.85" r="0.68" fill="currentColor" stroke="none" />
      <circle cx="13.35" cy="13.75" r="0.68" fill="currentColor" stroke="none" />
      <circle cx="17.25" cy="13.75" r="0.68" fill="currentColor" stroke="none" />
    </g>
  );
}
