import { Loader2, RefreshCw } from "../../../shared/ui/icons";
import { useId } from "react";
import { api } from "../../../api/client";
import { useDraftProbe } from "../model/use-draft-probe";
import type { ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkSelect, SkTextInput } from "../kit";
import { ProviderApiKeysField } from "../provider-api-keys-field";
import { endpointForProbe } from "./endpoint-for-probe";
import { ImageConnectionTest } from "./image-connection-test";
import { imageProtocolOptions } from "./image-endpoint-options";
import "../model/provider-connection-test.css";

type ImageEndpointFieldsProps = {
  endpoint: ModelEndpointConfig;
  keys: ModelEndpointApiKey[];
  selectedKey?: string;
  secretSentinel: string;
  onPatch: (patch: Partial<Omit<ModelEndpointConfig, "id" | "kind">>) => void;
  onKeysChange: (patch: { api_keys: ModelEndpointApiKey[]; api_key_selected?: string; api_key_balance: boolean }) => void;
  onRevealKey: (keyId: string) => Promise<string>;
};

/**
 * 编辑生图接入。分组顺序与普通供应商、Jev 接入一致：接入点、凭据、连通性，名称收在身份里。
 *
 * @param props 当前接入、密钥与修改回调
 * @returns 接入编辑表单
 */
export function ImageEndpointFields({
  endpoint,
  keys,
  selectedKey,
  secretSentinel,
  onPatch,
  onKeysChange,
  onRevealKey
}: ImageEndpointFieldsProps) {
  const { t } = useI18n();
  const probeFormId = useId();
  const target = endpointForProbe(endpoint, keys, selectedKey, secretSentinel);
  const { running: fetching, error: fetchError, run: fetchCatalog } = useDraftProbe(
    JSON.stringify(target), () => api.imageModels.models(target),
    ["Failed to fetch image models", "获取生图模型失败"]
  );
  const models = endpoint.models ?? [];
  const modelOptions = [...new Set([endpoint.model, ...models].filter(Boolean))].map((model) => ({ value: model, label: model }));
  const activeModel = endpoint.model || models[0] || "";

  /**
   * 用当前草稿获取模型目录，并写回可选模型。
   *
   * @returns 无
   */
  const fetchModels = async () => {
    const response = await fetchCatalog("models");
    if (response) onPatch({ models: response.models, model: endpoint.model || response.models[0] || "" });
  };

  return (
    <>
      <SettingsPanel
        title={t("Endpoint", "接入点")}
        description={t("Where image requests go, which format they use, and which model draws them.", "生图请求发往哪里、使用哪种格式、由哪个模型绘制。")}
      >
        <FieldGrid>
          <SettingsField label={t("API address", "API 地址")} anchor="image-models.endpoint" configKey="model_endpoints.endpoint" hint={t("Complete HTTP(S) URL, including any version path and port.", "完整 HTTP(S) 地址，包含版本路径与端口。")}>
            <SkTextInput
              form={probeFormId}
              title={t("Press Enter to test connection", "按回车测试连接")}
              value={endpoint.endpoint}
              onChange={(value) => onPatch({ endpoint: value })}
              placeholder="https://example.com/v1/images/generations"
              spellCheck={false}
            />
          </SettingsField>
          <SettingsField label={t("Model", "模型")} anchor="image-models.model" configKey="model_endpoints.model" hint={t("The model field sent with every image request.", "每次生图请求都会发送此模型字段。")}>
            {modelOptions.length > 0 ? (
              <SkSelect value={activeModel} options={modelOptions} onChange={(model) => onPatch({ model })} ariaLabel={t("Image model", "生图模型")} />
            ) : (
              <SkTextInput value={endpoint.model} onChange={(value) => onPatch({ model: value })} placeholder="gpt-image-1" spellCheck={false} />
            )}
          </SettingsField>
          <SettingsField label={t("Protocol", "协议")} anchor="image-models.protocol" configKey="model_endpoints.protocol" hint={t("Auto adapts the URL, body, and authentication; choose a format to override it.", "自动模式会适配地址、请求体和认证方式，也可以手动指定格式。")}>
            <SkSelect
              value={endpoint.protocol ?? "auto"}
              options={imageProtocolOptions()}
              onChange={(protocol) => onPatch({ protocol })}
              ariaLabel={t("Image request format", "生图请求格式")}
              menuMinimumWidth={230}
            />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>

      <SettingsPanel
        title={t("Credentials", "凭据")}
        id="settings-field-image-models-api_keys"
        description={t("API keys for this image connection, with optional load balancing across multiple keys.", "当前生图接入的 API 密钥，多密钥时可启用负载均衡。")}
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
          />
          <small className="sk-field-hint">{t("Use one selected key by default, or enable load balancing when multiple keys are configured. Environment variables can be referenced with `$env:VARIABLE_NAME`.", "默认使用一个选中的密钥；配置多个密钥后可以启用负载均衡。支持使用 `$env:VARIABLE_NAME` 引用环境变量。")}</small>
        </div>
      </SettingsPanel>

      <SettingsPanel
        title={t("Model catalog", "模型目录")}
        description={models.length > 0 ? t(`${models.length} models imported from this endpoint.`, `已从该端点导入 ${models.length} 个模型。`) : t("Fetch the endpoint catalog, then choose the model above.", "获取端点模型目录后，可以在上方选择模型。")}
      >
        <div className="provider-probe">
          <div className="provider-probe-actions">
            <Button className="provider-probe-run" disabled={fetching || !endpoint.endpoint.trim()} onClick={() => void fetchModels()}>
              {fetching ? <Loader2 size={14} className="provider-probe-spin" /> : <RefreshCw size={14} />}
              {fetching ? t("Fetching", "获取中") : t("Fetch models", "获取模型")}
            </Button>
          </div>
          {fetchError && <p className="provider-probe-error">{fetchError}</p>}
          {models.length > 0 && <div className="model-endpoint-models">{models.map((model) => <Button key={model} size="small" variant={model === activeModel ? "primary" : "secondary"} aria-pressed={model === activeModel} onClick={() => onPatch({ model })}>{model}</Button>)}</div>}
        </div>
      </SettingsPanel>

      <SettingsPanel
        title={t("Connectivity", "连通性")}
        description={t("Send a small real image request to verify the address, key, and model.", "发送一次小尺寸真实生图请求，验证地址、密钥与模型。")}
      >
        <div className="grid min-w-0 gap-2">
          <ImageConnectionTest
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
        description={t("Name shown in the image model menu.", "显示在生图模型菜单中的名称。")}
      >
        <FieldGrid>
          <SettingsField label={t("Display name", "显示名称")} anchor="image-models.name" configKey="model_endpoints.name" hint={t("Shown in the chat image model menu.", "显示在聊天页的生图模型菜单中。")}>
            <SkTextInput value={endpoint.name} onChange={(value) => onPatch({ name: value })} />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>
    </>
  );
}
