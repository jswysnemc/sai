import type { ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
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

  return (
    <>
      <SettingsGroup
        title={t("Endpoint", "接入点")}
        description={t("Where Jev requests go and which model answers them.", "Jev 请求发往哪里、由哪个模型回答。")}
      >
        <div className="settings-form-grid">
          <label className="settings-field full">
            <span>{t("API address", "API 地址")}</span>
            <input
              value={endpoint.endpoint}
              onChange={(event) => onPatch({ endpoint: event.target.value })}
              placeholder="https://api.typesafe.ai/v1/systemone"
              spellCheck={false}
            />
            <small>{t("A TypeSafe base URL ending in /v1 is completed to /v1/systemone.", "以 /v1 结尾的 TypeSafe 根地址会自动补全为 /v1/systemone。")}</small>
          </label>
          <label className="settings-field">
            <span>{t("Model", "模型")}</span>
            <input
              value={endpoint.model}
              onChange={(event) => onPatch({ model: event.target.value })}
              placeholder="jev-latest"
              spellCheck={false}
            />
            <small>{t("Leave blank to use jev-latest.", "留空使用 jev-latest。")}</small>
          </label>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Credentials", "凭据")}
        description={t("API keys for this Jev connection. An empty key reads TYPESAFE_API_KEY, then TYPESAFE_KEY.", "当前 Jev 接入的 API 密钥。留空时依次读取 TYPESAFE_API_KEY、TYPESAFE_KEY。")}
      >
        <div className="settings-field full">
          <ProviderApiKeysField
            key={endpoint.id}
            providerId={endpoint.id}
            keys={keys}
            selected={selectedKey}
            balance={endpoint.api_key_balance === true}
            secretSentinel={secretSentinel}
            onRevealKey={onRevealKey}
            onChange={onKeysChange}
          />
          <small>{t("Use one selected key by default, or enable load balancing when multiple keys are configured. Environment variables can be referenced with `$env:VARIABLE_NAME`.", "默认使用一个选中的密钥；配置多个密钥后可以启用负载均衡。支持使用 `$env:VARIABLE_NAME` 引用环境变量。")}</small>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Connectivity", "连通性")}
        description={t("Ask the Jev model one minimal question to verify the address, key, and model.", "向 Jev 模型发送一个最小问题，验证地址、密钥与模型。")}
      >
        <div className="settings-field full">
          <JevConnectionTest
            key={`${endpoint.id}:${endpoint.model}:${selectedKey ?? ""}`}
            endpoint={endpoint}
            keys={keys}
            selectedKey={selectedKey}
            secretSentinel={secretSentinel}
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        collapsible
        defaultOpen={endpoint.name.trim().length === 0}
        title={t("Identity", "身份")}
        description={t("Name shown in the Jev connection list.", "显示在 Jev 接入列表中的名称。")}
      >
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Display name", "显示名称")}</span>
            <input value={endpoint.name} onChange={(event) => onPatch({ name: event.target.value })} />
            <small>{t("Used when choosing which connection the built-in Jev features call.", "选择内置 Jev 功能使用哪条接入时显示此名称。")}</small>
          </label>
        </div>
      </SettingsGroup>
    </>
  );
}
