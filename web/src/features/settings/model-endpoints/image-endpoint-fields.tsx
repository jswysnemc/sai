import { CheckCircle2, Loader2, PlugZap, RefreshCw, XCircle } from "lucide-react";
import type { ImageEndpointProbeReport, ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
import { ProviderApiKeysField } from "../provider-api-keys-field";
import { imageProtocolOptions } from "./image-endpoint-options";

type ImageEndpointFieldsProps = {
  endpoint: ModelEndpointConfig;
  keys: ModelEndpointApiKey[];
  selectedKey?: string;
  secretSentinel: string;
  models: string[];
  fetching: boolean;
  probing: boolean;
  probe: ImageEndpointProbeReport | null;
  fetchError: string;
  probeError: string;
  onPatch: (patch: Partial<ModelEndpointConfig>) => void;
  onKeysChange: (patch: { api_keys: ModelEndpointApiKey[]; api_key_selected?: string; api_key_balance: boolean }) => void;
  onRevealKey: (keyId: string) => Promise<string>;
  onFetchModels: () => void;
  onProbe: () => void;
};

/** 渲染生图端点的连接、模型目录、密钥和真实探测分组。 */
export function ImageEndpointFields(props: ImageEndpointFieldsProps) {
  const { t } = useI18n();
  const modelOptions = props.models.map((model) => ({ value: model, label: model }));
  const activeModel = props.endpoint.model || props.models[0] || "";

  return (
    <>
      <SettingsGroup
        title={t("Connection", "连接配置")}
        description={t("Use the complete image request URL and the model identifier expected by the endpoint.", "填写完整的图片请求地址，以及该接口要求的模型标识。")}
      >
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Name", "名称")}</span>
            <input value={props.endpoint.name} onChange={(event) => props.onPatch({ name: event.target.value })} />
            <small>{t("Shown in the chat image model menu.", "显示在聊天页的生图模型菜单中。")}</small>
          </label>
          <div className="settings-field">
            <span>{t("Image format", "生图格式")}</span>
            <Select
              value={props.endpoint.protocol ?? "auto"}
              options={imageProtocolOptions()}
              onChange={(protocol) => props.onPatch({ protocol })}
              ariaLabel={t("Image request format", "生图请求格式")}
              menuMinimumWidth={230}
            />
            <small>{t("Auto adapts the URL, body, and authentication; choose a format to override it.", "自动模式会适配地址、请求体和认证方式，也可以手动指定格式。")}</small>
          </div>
          <label className="settings-field">
            <span>{t("Model ID", "模型标识")}</span>
            {modelOptions.length > 0 ? (
              <Select value={activeModel} options={modelOptions} onChange={(model) => props.onPatch({ model })} ariaLabel={t("Image model", "生图模型")} />
            ) : (
              <input value={props.endpoint.model} onChange={(event) => props.onPatch({ model: event.target.value })} placeholder="gpt-image-1" spellCheck={false} />
            )}
            <small>{t("The model field sent with every image request.", "每次生图请求都会发送此模型字段。")}</small>
          </label>
          <label className="settings-field full">
            <span>{t("Request endpoint", "请求地址")}</span>
            <input value={props.endpoint.endpoint} onChange={(event) => props.onPatch({ endpoint: event.target.value })} placeholder="https://example.com/v1/images/generations" spellCheck={false} />
            <small>{t("Complete HTTP(S) URL, including any version path and port.", "完整 HTTP(S) 地址，包含版本路径与端口。")}</small>
          </label>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Credentials", "凭据")}
        description={t("Use the same stable multi-key editor as regular model providers.", "复用普通模型供应商的多密钥编辑器。")}
      >
        <div className="settings-field full">
          <ProviderApiKeysField
            key={props.endpoint.id}
            providerId={props.endpoint.id}
            keys={props.keys}
            selected={props.selectedKey}
            balance={props.endpoint.api_key_balance === true}
            secretSentinel={props.secretSentinel}
            onRevealKey={props.onRevealKey}
            onChange={props.onKeysChange}
          />
          <small>{t("Choose one key or rotate requests across all configured keys. Environment variables are supported.", "可以固定使用一个密钥，也可以在全部密钥间轮换。支持环境变量引用。")}</small>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Model catalog", "模型目录")}
        description={props.models.length > 0 ? t(`${props.models.length} models imported from this endpoint.`, `已从该端点导入 ${props.models.length} 个模型。`) : t("Fetch the endpoint catalog, then choose the model above.", "获取端点模型目录后，可以在上方选择模型。")}
        actions={<Button className="settings-secondary" disabled={props.fetching || !props.endpoint.endpoint.trim()} onClick={props.onFetchModels}><RefreshCw size={14} className={props.fetching ? "spin" : undefined} />{props.fetching ? t("Fetching", "获取中") : t("Fetch models", "获取模型")}</Button>}
      >
        {props.fetchError && <p className="settings-inline-error">{props.fetchError}</p>}
        {props.models.length > 0 && <div className="model-endpoint-models">{props.models.map((model) => <span key={model}>{model}</span>)}</div>}
      </SettingsGroup>

      <SettingsGroup
        title={t("Connectivity", "连通性")}
        description={t("Send a small real image request to verify the endpoint and selected key.", "发送一次小尺寸真实生图请求，验证端点与当前密钥。")}
        actions={<Button className="settings-secondary" disabled={props.probing || !props.endpoint.endpoint.trim() || !activeModel} onClick={props.onProbe}>{props.probing ? <Loader2 size={14} className="spin" /> : <PlugZap size={14} />}{props.probing ? t("Testing", "测试中") : t("Test image generation", "测试生图")}</Button>}
      >
        {props.probeError && <p className="settings-inline-error">{props.probeError}</p>}
        {props.probe && <div className={props.probe.ok ? "image-endpoint-probe ok" : "image-endpoint-probe failed"}>
          {props.probe.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
          <span>{props.probe.ok ? t("Image response received", "已收到图片响应") : t("Image request failed", "图片请求失败")}</span>
          <em>{props.probe.total_ms} ms</em>
          {props.probe.stages[0]?.detail && <small title={props.probe.stages[0].detail}>{props.probe.stages[0].detail}</small>}
        </div>}
      </SettingsGroup>
    </>
  );
}
