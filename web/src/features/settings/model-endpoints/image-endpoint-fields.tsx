import { Loader2, RefreshCw } from "../../../shared/ui/icons";
import { useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
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
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");
  const models = endpoint.models ?? [];
  const modelOptions = models.map((model) => ({ value: model, label: model }));
  const activeModel = endpoint.model || models[0] || "";

  /**
   * 用当前草稿拉取模型目录，并写回可选模型。
   *
   * @returns 无
   */
  const fetchModels = async () => {
    setFetching(true);
    setFetchError("");
    try {
      const response = await api.imageModels.models(endpointForProbe(endpoint, keys, selectedKey, secretSentinel));
      onPatch({ models: response.models, model: endpoint.model || response.models[0] || "" });
    } catch (cause) {
      setFetchError(toDisplayError(cause, "Failed to fetch image models", "获取生图模型失败").message);
    } finally {
      setFetching(false);
    }
  };

  return (
    <>
      <SettingsGroup
        title={t("Endpoint", "接入点")}
        description={t("Where image requests go, which format they use, and which model draws them.", "生图请求发往哪里、使用哪种格式、由哪个模型绘制。")}
      >
        <div className="settings-form-grid">
          <label className="settings-field full">
            <span>{t("API address", "API 地址")}</span>
            <input
              value={endpoint.endpoint}
              onChange={(event) => onPatch({ endpoint: event.target.value })}
              placeholder="https://example.com/v1/images/generations"
              spellCheck={false}
            />
            <small>{t("Complete HTTP(S) URL, including any version path and port.", "完整 HTTP(S) 地址，包含版本路径与端口。")}</small>
          </label>
          <label className="settings-field">
            <span>{t("Model", "模型")}</span>
            {modelOptions.length > 0 ? (
              <Select value={activeModel} options={modelOptions} onChange={(model) => onPatch({ model })} ariaLabel={t("Image model", "生图模型")} />
            ) : (
              <input value={endpoint.model} onChange={(event) => onPatch({ model: event.target.value })} placeholder="gpt-image-1" spellCheck={false} />
            )}
            <small>{t("The model field sent with every image request.", "每次生图请求都会发送此模型字段。")}</small>
          </label>
          <div className="settings-field">
            <span>{t("Protocol", "协议")}</span>
            <Select
              value={endpoint.protocol ?? "auto"}
              options={imageProtocolOptions()}
              onChange={(protocol) => onPatch({ protocol })}
              ariaLabel={t("Image request format", "生图请求格式")}
              menuMinimumWidth={230}
            />
            <small>{t("Auto adapts the URL, body, and authentication; choose a format to override it.", "自动模式会适配地址、请求体和认证方式，也可以手动指定格式。")}</small>
          </div>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Credentials", "凭据")}
        description={t("API keys for this image connection, with optional load balancing across multiple keys.", "当前生图接入的 API 密钥，多密钥时可启用负载均衡。")}
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
          {models.length > 0 && <div className="model-endpoint-models">{models.map((model) => <span key={model}>{model}</span>)}</div>}
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Connectivity", "连通性")}
        description={t("Send a small real image request to verify the address, key, and model.", "发送一次小尺寸真实生图请求，验证地址、密钥与模型。")}
      >
        <div className="settings-field full">
          <ImageConnectionTest
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
        description={t("Name shown in the image model menu.", "显示在生图模型菜单中的名称。")}
      >
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Display name", "显示名称")}</span>
            <input value={endpoint.name} onChange={(event) => onPatch({ name: event.target.value })} />
            <small>{t("Shown in the chat image model menu.", "显示在聊天页的生图模型菜单中。")}</small>
          </label>
        </div>
      </SettingsGroup>
    </>
  );
}
