import { useState } from "react";
import { QrCode } from "../../shared/ui/icons";
import type { AppConfig, WeixinLoginAccount } from "../../api/contracts";
import { GatewayRuntimeControl } from "../gateways/gateway-runtime-control";
import { GatewayBrandIcon } from "../gateways/gateway-brand-icon";
import { WeixinLoginDialog } from "../gateways/weixin-login-dialog";
import { InlineSwitch, SettingsPanel } from "./kit";
import { GatewayFields } from "./gateways/gateway-fields";
import { Button } from "../../shared/ui/button/button";
import type { GatewayId } from "./settings-types";
import { useI18n } from "../i18n/use-i18n";

type GatewaySettingsSectionProps = {
  config: AppConfig;
  dirty: boolean;
  secretSentinel: string;
  onGatewayChange: (gateway: GatewayId, patch: Record<string, unknown>) => void;
  onSave: () => Promise<void>;
};

/**
 * 渲染 QQ 与微信网关的配置和运行控制。
 *
 * @param props 网关配置、更新回调和保存回调
 * @returns 网关设置区域
 */
export function GatewaySettingsSection({ config, dirty, secretSentinel, onGatewayChange, onSave }: GatewaySettingsSectionProps) {
  const { t } = useI18n();
  const weixin = config.gateways.weixin;
  const [loginOpen, setLoginOpen] = useState(false);

  /** 登录成功后回填微信账户配置。 */
  const handleConfirmed = (account: WeixinLoginAccount) => {
    onGatewayChange("weixin", {
      enabled: true,
      account: account.account_id,
      base_url: account.base_url,
      cdn_base_url: account.cdn_base_url,
      token: ""
    });
  };

  return <div className="grid gap-4">
    {(["qq", "weixin"] as const).map((gateway) => <SettingsPanel key={gateway}
      title={gateway === "qq" ? "QQ" : t("Weixin", "微信")}
      icon={<GatewayBrandIcon gatewayId={gateway} size={16} />}
      actions={<>
        <InlineSwitch label={t("Enabled", "已启用")} checked={config.gateways[gateway].enabled} onChange={(enabled) => onGatewayChange(gateway, { enabled })} />
        <GatewayRuntimeControl gatewayId={gateway} enabled={config.gateways[gateway].enabled} dirty={dirty} onSave={onSave} />
      </>}
    >
      <GatewayFields secretSentinel={secretSentinel} gateway={gateway} config={config} onChange={(patch) => onGatewayChange(gateway, patch)} />
      {gateway === "weixin" && <div className="flex flex-wrap items-center gap-2 text-xs text-muted">
        <Button variant="secondary" onClick={() => setLoginOpen(true)}><QrCode size={14} />{t("Scan QR code to log in", "扫码登录")}</Button>
        <span>{t("Account credentials are filled after login.", "登录后回填账户凭据。")}</span>
      </div>}
    </SettingsPanel>)}
    <WeixinLoginDialog open={loginOpen} baseUrl={weixin.base_url} botType={weixin.bot_type} onClose={() => setLoginOpen(false)} onConfirmed={handleConfirmed} />
  </div>;
}
