import { useState } from "react";
import { QrCode } from "lucide-react";
import type { AppConfig, WeixinLoginAccount } from "../../api/contracts";
import { GatewayRuntimeControl } from "../gateways/gateway-runtime-control";
import { WeixinLoginDialog } from "../gateways/weixin-login-dialog";
import { SettingsGroup } from "./editor-layout";
import type { GatewayId } from "./settings-types";
import { PasswordField } from "../../shared/ui/password-field";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";

type GatewaySettingsSectionProps = {
  config: AppConfig;
  dirty: boolean;
  onGatewayChange: (gateway: GatewayId, patch: Record<string, unknown>) => void;
  onSave: () => Promise<void>;
};

/**
 * 渲染 QQ 与微信网关的配置和运行控制。
 *
 * @param props 网关配置、更新回调和保存回调
 * @returns 网关设置区域
 */
export function GatewaySettingsSection({ config, dirty, onGatewayChange, onSave }: GatewaySettingsSectionProps) {
  const { t } = useI18n();
  const qq = config.gateways.qq;
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

  return (
    <div className="settings-editor gateway-settings">
      <SettingsGroup
        title="QQ"
        description={t("Configure QQ bot transport and authentication.", "配置 QQ 机器人监听方式和认证信息。")}
        actions={<label className="settings-switch"><input type="checkbox" checked={qq.enabled} onChange={(event) => onGatewayChange("qq", { enabled: event.target.checked })} /><span /><strong>{qq.enabled ? t("Enabled", "已启用") : t("Disabled", "未启用")}</strong></label>}
      >
        <div className="settings-form-grid">
          <div className="settings-field"><span>{t("Transport", "传输方式")}</span><Select value={qq.transport} options={TRANSPORT_OPTIONS} onChange={(value) => onGatewayChange("qq", { transport: value })} ariaLabel={t("QQ transport", "QQ 传输方式")} /><small>{t("Choose according to the bot connection method", "根据机器人接入方式选择")}</small></div>
          <label className="settings-field"><span>{t("Listen address", "监听地址")}</span><input value={qq.listen} onChange={(event) => onGatewayChange("qq", { listen: event.target.value })} spellCheck={false} /><small>{t("Local service bind address", "本地服务绑定地址")}</small></label>
          <label className="settings-field full"><span>{t("API address", "API 地址")}</span><input value={qq.base_url} onChange={(event) => onGatewayChange("qq", { base_url: event.target.value })} spellCheck={false} /></label>
          <label className="settings-field"><span>App ID</span><input value={qq.app_id} onChange={(event) => onGatewayChange("qq", { app_id: event.target.value })} /></label>
          <div className="settings-field"><span>Client Secret</span><PasswordField value={qq.client_secret} onChange={(value) => onGatewayChange("qq", { client_secret: value })} /></div>
          <div className="settings-field full"><span>{t("Compatibility token", "兼容令牌")}</span><PasswordField value={qq.token} onChange={(value) => onGatewayChange("qq", { token: value })} /><small>{t("Use the `AppID:AppSecret` format when required", "需要时使用 `AppID:AppSecret` 格式")}</small></div>
        </div>
        <GatewayRuntimeControl gatewayId="qq" enabled={qq.enabled} dirty={dirty} onSave={onSave} />
      </SettingsGroup>
      <SettingsGroup
        title={t("Weixin", "微信")}
        description={t("Configure the Weixin bot service, account, and access token.", "配置微信机器人服务、账户和访问令牌。")}
        actions={<label className="settings-switch"><input type="checkbox" checked={weixin.enabled} onChange={(event) => onGatewayChange("weixin", { enabled: event.target.checked })} /><span /><strong>{weixin.enabled ? t("Enabled", "已启用") : t("Disabled", "未启用")}</strong></label>}
      >
        <div className="settings-form-grid">
          <label className="settings-field full"><span>{t("API address", "API 地址")}</span><input value={weixin.base_url} onChange={(event) => onGatewayChange("weixin", { base_url: event.target.value })} spellCheck={false} /></label>
          <label className="settings-field full"><span>{t("CDN address", "CDN 地址")}</span><input value={weixin.cdn_base_url} onChange={(event) => onGatewayChange("weixin", { cdn_base_url: event.target.value })} spellCheck={false} /></label>
          <label className="settings-field"><span>{t("Bot type", "机器人类型")}</span><input value={weixin.bot_type} onChange={(event) => onGatewayChange("weixin", { bot_type: event.target.value })} /></label>
          <label className="settings-field"><span>{t("Account", "账户")}</span><input value={weixin.account} onChange={(event) => onGatewayChange("weixin", { account: event.target.value })} /></label>
          <label className="settings-field"><span>Agent</span><input value={weixin.bot_agent} onChange={(event) => onGatewayChange("weixin", { bot_agent: event.target.value })} /></label>
          <div className="settings-field"><span>{t("Access token", "访问令牌")}</span><PasswordField value={weixin.token} onChange={(value) => onGatewayChange("weixin", { token: value })} /></div>
        </div>
        <div className="gateway-weixin-login-row">
          <button type="button" className="gateway-weixin-login-button" onClick={() => setLoginOpen(true)}><QrCode size={14} />{t("Scan QR code to log in", "扫码登录")}</button>
          <small>{t("Account credentials are retrieved and filled after QR-code login", "扫码登录后自动获取并回填账户凭证")}</small>
        </div>
        <GatewayRuntimeControl gatewayId="weixin" enabled={weixin.enabled} dirty={dirty} onSave={onSave} />
      </SettingsGroup>
      <WeixinLoginDialog
        open={loginOpen}
        baseUrl={weixin.base_url}
        botType={weixin.bot_type}
        onClose={() => setLoginOpen(false)}
        onConfirmed={handleConfirmed}
      />
    </div>
  );
}

const TRANSPORT_OPTIONS = [
  { value: "webhook", label: "Webhook" },
  { value: "websocket", label: "WebSocket" }
];
