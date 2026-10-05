import { useEffect, useState } from "react";
import type { ProviderConfig } from "../../api/contracts";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { Loader2, RefreshCw } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";
import { SettingsField, SkSecretInput, SkSelect, SkTextInput } from "../settings/kit";
import { protocolOptions } from "../settings/providers/provider-options";
import "../settings/model/provider-connection-test.css";
import type { ProviderSetupDraft } from "./provider-setup-draft";

type ProviderSetupFormProps = {
  providers: ProviderConfig[];
  draft: ProviderSetupDraft;
  saving: boolean;
  onSelect: (id: string) => void;
  onChange: (patch: Partial<ProviderSetupDraft>) => void;
  onSubmit: () => void;
};

/**
 * 【首次配置】【连接表单】收集供应商、地址、协议、密钥，并可探测模型目录。
 * @param props 模板列表、草稿和交互回调
 * @returns 响应式配置表单
 */
export function ProviderSetupForm({ providers, draft, saving, onSelect, onChange, onSubmit }: ProviderSetupFormProps) {
  const { t } = useI18n();
  const builtInFree = draft.provider_id === "opencode" && draft.base_url.replace(/\/$/, "") === "https://opencode.ai/zen/v1";
  const [models, setModels] = useState<string[]>([]);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);

  useEffect(() => {
    setModels([]);
    setFetchError(null);
  }, [draft.provider_id, draft.base_url]);

  /**
   * 用当前草稿探测模型目录，可选手动填写模型。
   * @returns 完成后的 Promise
   */
  const fetchModels = async () => {
    setFetching(true);
    setFetchError(null);
    try {
      const response = await api.providers.models({
        id: draft.provider_id || "setup",
        display_name: draft.display_name || "setup",
        base_url: draft.base_url,
        protocol: draft.protocol,
        api_key: draft.api_key || undefined,
        default_model: draft.model || undefined
      });
      setModels(response.models);
      if (!draft.model && response.models[0]) onChange({ model: response.models[0] });
    } catch (reason) {
      setFetchError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setFetching(false);
    }
  };

  const modelOptions = [...new Set([draft.model, ...models].filter(Boolean))].map((model) => ({ value: model, label: model }));

  return (
    <form className="grid grid-cols-1 gap-4 sm:grid-cols-2" onSubmit={event => { event.preventDefault(); onSubmit(); }}>
      <SettingsField label={t("Provider", "供应商")} className="sm:col-span-2">
        <SkSelect value={draft.provider_id ?? ""} disabled={saving} onChange={onSelect} options={[
          ...providers.map(provider => ({ value: provider.id, label: provider.display_name })),
          { value: "", label: t("Custom compatible provider", "自定义兼容供应商") }
        ]} />
      </SettingsField>
      <SettingsField label={t("Provider name", "供应商名称")}>
        <SkTextInput disabled={saving} value={draft.display_name} onChange={display_name => onChange({ display_name })} />
      </SettingsField>
      <SettingsField label={t("Protocol", "协议")}>
        <SkSelect disabled={saving} value={draft.protocol} options={protocolOptions()} onChange={protocol => onChange({ protocol })} />
      </SettingsField>
      <SettingsField label={t("API address", "API 地址")} className="sm:col-span-2" hint={t("Include the API version path, for example https://api.example.com/v1.", "包含接口版本路径，例如 https://api.example.com/v1。") }>
        <SkTextInput disabled={saving} mono value={draft.base_url} placeholder="https://api.example.com/v1" onChange={base_url => onChange({ base_url })} />
      </SettingsField>
      <SettingsField label={t("API key", "API 密钥")} className="sm:col-span-2" hint={builtInFree
        ? t("The built-in free provider does not require an API key.", "内置免费供应商无需填写 API 密钥。")
        : t("Enter a key or $env:VARIABLE. Leave blank to keep existing credentials.", "填写密钥或 $env:VARIABLE。留空保留已有凭据。") }>
        <SkSecretInput requireExplicitEdit={false} disabled={saving} value={draft.api_key} onChange={api_key => onChange({ api_key })} />
      </SettingsField>
      <SettingsField label={t("Default model", "默认模型")} className="sm:col-span-2" hint={t("Fetch the catalog after the address and key are ready, or type a model ID.", "地址和密钥就绪后可获取目录，也可直接填写模型 ID。") }>
        {modelOptions.length > 0 ? (
          <SkSelect value={draft.model} options={modelOptions} disabled={saving} onChange={model => onChange({ model })} />
        ) : (
          <SkTextInput disabled={saving} mono value={draft.model} placeholder={t("Model ID", "模型 ID")} onChange={model => onChange({ model })} />
        )}
      </SettingsField>
      <div className="flex flex-wrap items-center justify-between gap-2 sm:col-span-2">
        <Button type="button" variant="secondary" disabled={saving || fetching || !draft.base_url.trim()} onClick={() => void fetchModels()}>
          {fetching ? <Loader2 size={14} className="provider-probe-spin" /> : <RefreshCw size={14} />}
          {fetching ? t("Fetching", "获取中") : t("Fetch models", "获取模型")}
        </Button>
        <Button type="submit" variant="primary" disabled={saving}>{saving ? t("Saving…", "正在保存…") : t("Save and start", "保存并开始使用")}</Button>
      </div>
      {fetchError && <p className="sm:col-span-2 m-0 text-sm text-[var(--danger)]">{fetchError}</p>}
      {models.length > 0 && (
        <div className="flex flex-wrap gap-2 sm:col-span-2">
          {models.slice(0, 12).map((model) => (
            <Button key={model} type="button" size="small" variant={model === draft.model ? "primary" : "secondary"} onClick={() => onChange({ model })}>{model}</Button>
          ))}
        </div>
      )}
    </form>
  );
}
