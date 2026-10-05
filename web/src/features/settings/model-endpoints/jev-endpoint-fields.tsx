import { useId } from "react";
import type { ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkSelect, SkTextInput } from "../kit";
import { ProviderApiKeysField } from "../provider-api-keys-field";
import { JevConnectionTest } from "./jev-connection-test";

type JevEndpointFieldsProps = {
  endpoint: ModelEndpointConfig;
  keys: ModelEndpointApiKey[];
  selectedKey?: string;
  secretSentinel: string;
  onPatch: (patch: Partial<Omit<ModelEndpointConfig, "id" | "kind">>) => void;
  onKeysChange: (patch: { api_keys: ModelEndpointApiKey[]; api_key_selected?: string; api_key_balance: boolean }) => void;
  onRevealKey: (keyId: string) => Promise<string>;
};

/**
 * 编辑 Jev 接入。分组顺序与普通供应商的连接页一致：接入点、凭据、连通性，名称收在身份里。
 *
 * @param props 当前接入、密钥与修改回调
 * @returns 接入编辑表单
 */
export function JevEndpointFields({
  endpoint,
  keys,
  selectedKey,
  secretSentinel,
  onPatch,
  onKeysChange,
  onRevealKey
}: JevEndpointFieldsProps) {
  const { t } = useI18n();
  const probeFormId = useId();

  return (
    <>
      <SettingsPanel
        title={t("Endpoint", "接入点")}
        description={t("Where Jev requests go and which model answers them.", "Jev 请求发往哪里、由哪个模型回答。")}
      >
        <FieldGrid>
          <SettingsField label={t("API address", "API 地址")} anchor="jev.endpoint" configKey="model_endpoints.endpoint" hint={t("A TypeSafe base URL ending in /v1 is completed to /v1/systemone.", "以 /v1 结尾的 TypeSafe 根地址会自动补全为 /v1/systemone。")}>
            <SkTextInput
              form={probeFormId}
              title={t("Press Enter to test connection", "按回车测试连接")}
              value={endpoint.endpoint}
              onChange={(value) => onPatch({ endpoint: value })}
              placeholder="https://api.typesafe.ai/v1/systemone"
              spellCheck={false}
            />
          </SettingsField>
          <SettingsField label={t("Model", "模型")} anchor="jev.model" configKey="model_endpoints.model" hint={t("Leave blank to use jev-latest.", "留空使用 jev-latest。")}>
            <SkTextInput
              value={endpoint.model}
              onChange={(value) => onPatch({ model: value })}
              placeholder="jev-latest"
              spellCheck={false}
            />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>

      <SettingsPanel
        title={t("Credentials", "凭据")}
        id="settings-field-jev-api_keys"
        description={t("API keys for this Jev connection. An empty key reads TYPESAFE_API_KEY, then TYPESAFE_KEY.", "当前 Jev 接入的 API 密钥。留空时依次读取 TYPESAFE_API_KEY、TYPESAFE_KEY。")}
      >
        <div className="grid min-w-0 gap-2">
          <ProviderApiKeysField
            key={endpoint.id}
            providerId={endpoint.id}
            keys={keys}
            selected={selectedKey}
            balance={endpoint.api_key_balance === true}
            secretSentinel={secretSentinel}
            onRevealKey={onRevealKey}
            onChange={onKeysChange}
            compact
          />
          <small className="sk-field-hint">{t("Use one selected key by default, or enable load balancing when multiple keys are configured. Environment variables can be referenced with `$env:VARIABLE_NAME`.", "默认使用一个选中的密钥；配置多个密钥后可以启用负载均衡。支持使用 `$env:VARIABLE_NAME` 引用环境变量。")}</small>
        </div>
      </SettingsPanel>

      <SettingsPanel
        title={t("Connectivity", "连通性")}
        description={t("Ask the Jev model one minimal question to verify the address, key, and model.", "向 Jev 模型发送一个最小问题，验证地址、密钥与模型。")}
      >
        <div className="grid min-w-0 gap-2">
          <JevConnectionTest
            formId={probeFormId}
            key={`${endpoint.id}:${endpoint.model}:${selectedKey ?? ""}`}
            endpoint={endpoint}
            keys={keys}
            selectedKey={selectedKey}
            secretSentinel={secretSentinel}
          />
        </div>
      </SettingsPanel>

      <SettingsPanel
        title={t("Identity", "身份")}
        description={t("Name shown in the Jev connection list.", "显示在 Jev 接入列表中的名称。")}
      >
        <FieldGrid>
          <SettingsField label={t("Display name", "显示名称")} anchor="jev.name" configKey="model_endpoints.name" hint={t("Used when choosing which connection the built-in Jev features call.", "选择内置 Jev 功能使用哪条接入时显示此名称。")}>
            <SkTextInput value={endpoint.name} onChange={(value) => onPatch({ name: value })} />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>
    </>
  );
}
