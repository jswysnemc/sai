import type { AppConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SkSecretInput, SkTextInput } from "../kit";
import type { GatewayId } from "../settings-types";

type Props = { gateway: GatewayId; config: AppConfig; secretSentinel: string; onChange: (patch: Record<string, unknown>) => void };

/**
 * 【网关】【配置字段】按照平台展示接入与认证字段。
 * @param props 平台、配置与草稿更新回调
 * @returns 响应式字段栅格
 */
export function GatewayFields({ gateway, config, secretSentinel, onChange }: Props) {
  const { t } = useI18n();
  const value = config.gateways[gateway];
  const fields = gateway === "qq" ? [
    { key: "listen", en: "Listen address", zh: "监听地址", size: "md" },
    { key: "base_url", en: "API address", zh: "API 地址" },
    { key: "app_id", en: "App ID", zh: "应用 ID", size: "md" },
    { key: "client_secret", en: "Client secret", zh: "客户端密钥", secret: true },
    { key: "token", en: "Compatibility token", zh: "兼容令牌", secret: true, hint: t("Use AppID:AppSecret when required.", "需要时使用 AppID:AppSecret 格式。") }
  ] : [
    { key: "base_url", en: "API address", zh: "API 地址" },
    { key: "cdn_base_url", en: "CDN address", zh: "CDN 地址" },
    { key: "bot_type", en: "Bot type", zh: "机器人类型", size: "sm" },
    { key: "account", en: "Account", zh: "账户", size: "md" },
    { key: "bot_agent", en: "Agent", zh: "Agent", size: "md" },
    { key: "token", en: "Access token", zh: "访问令牌", secret: true }
  ];
  return <FieldGrid>
    {gateway === "qq" && <SettingsField label={t("Transport", "传输方式")} anchor="gateways.qq.transport"><ChoicePills value={config.gateways.qq.transport} options={[{ value: "webhook", label: "Webhook" }, { value: "websocket", label: "WebSocket" }]} onChange={(transport) => onChange({ transport })} /></SettingsField>}
    {fields.map((field) => <SettingsField key={field.key} label={t(field.en, field.zh)} anchor={`gateways.${gateway}.${field.key}`} configKey={`gateways.${gateway}.${field.key}`} size={field.size as "sm" | "md" | undefined} hint={field.hint}>
      {field.secret ? <SkSecretInput secretSentinel={secretSentinel} value={String(value[field.key as keyof typeof value] ?? "")} onChange={(next) => onChange({ [field.key]: next })} /> : <SkTextInput value={String(value[field.key as keyof typeof value] ?? "")} onChange={(next) => onChange({ [field.key]: next })} />}
    </SettingsField>)}
  </FieldGrid>;
}
