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
 * 渲染 QQ 官方企鹅剪影（Simple Icons 路径），小尺寸下品牌可识别。
 *
 * @returns QQ 图形节点
 */
function QqGlyph() {
  return (
    <path
      fill="currentColor"
      d="M21.395 15.035a39.548 39.548 0 0 0-.803-2.264l-1.079-2.695c.001-.032.014-.562.014-.836C19.526 4.632 17.351 0 12 0S4.474 4.632 4.474 9.241c0 .274.013.804.014.836l-1.08 2.695a38.97 38.97 0 0 0-.802 2.264c-1.021 3.283-.69 4.643-.438 4.673.54.065 2.103-2.472 2.103-2.472 0 1.469.756 3.387 2.394 4.771-.612.188-1.363.479-1.845.835-.434.32-.379.646-.301.778.343.578 5.883.369 7.482.189 1.6.18 7.14.389 7.483-.189.078-.132.132-.458-.301-.778-.483-.356-1.233-.646-1.846-.836 1.637-1.384 2.393-3.302 2.393-4.771 0 0 1.563 2.537 2.103 2.472.251-.03.581-1.39-.438-4.673"
    />
  );
}

/**
 * 渲染微信官方双气泡剪影（Simple Icons 路径），与 QQ 同为实心风格。
 *
 * @returns 微信图形节点
 */
function WeixinGlyph() {
  return (
    <path
      fill="currentColor"
      d="M8.691 2.188C3.891 2.188 0 5.476 0 9.53c0 2.212 1.17 4.203 3.002 5.55a.59.59 0 0 1 .213.665l-.39 1.48c-.019.07-.048.141-.048.213 0 .163.13.295.29.295a.326.326 0 0 0 .167-.054l1.903-1.114a.864.864 0 0 1 .717-.098 10.16 10.16 0 0 0 2.837.403c.276 0 .543-.027.811-.05-.857-2.578.157-4.972 1.932-6.446 1.703-1.415 3.882-1.98 5.853-1.838-.576-3.583-4.196-6.348-8.596-6.348zM5.785 5.991c.642 0 1.162.529 1.162 1.18a1.17 1.17 0 0 1-1.162 1.178A1.17 1.17 0 0 1 4.623 7.17c0-.651.52-1.18 1.162-1.18zm5.813 0c.642 0 1.162.529 1.162 1.18a1.17 1.17 0 0 1-1.162 1.178 1.17 1.17 0 0 1-1.162-1.178c0-.651.52-1.18 1.162-1.18zm5.34 2.867c-1.797-.052-3.746.512-5.28 1.786-1.72 1.428-2.687 3.72-1.78 6.22.942 2.453 3.666 4.229 6.884 4.229.826 0 1.622-.12 2.361-.336a.722.722 0 0 1 .598.082l1.584.926a.272.272 0 0 0 .14.047c.134 0 .24-.111.24-.247 0-.06-.023-.12-.038-.177l-.327-1.233a.582.582 0 0 1-.023-.156.49.49 0 0 1 .201-.398C23.024 18.48 24 16.82 24 14.98c0-3.21-2.931-5.837-6.656-6.088V8.89c-.135-.01-.27-.027-.406-.031zm-2.53 3.274c.535 0 .969.44.969.982a.976.976 0 0 1-.969.983.976.976 0 0 1-.969-.983c0-.542.434-.982.97-.982zm4.844 0c.535 0 .969.44.969.982a.976.976 0 0 1-.968.983.976.976 0 0 1-.969-.983c0-.542.433-.982.968-.982z"
    />
  );
}
