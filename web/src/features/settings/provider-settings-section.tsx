import { Check, Cpu, Plus, RefreshCw, Trash2 } from "lucide-react";
import { createElement, useEffect, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { AppConfig, ProviderConfig } from "../../api/contracts";
import { EditorHeader } from "./editor-layout";
import { ModelMetadataEditor } from "./model-metadata-editor";
import { ModelImportDialog } from "./model-import-dialog";
import { ObjectListPanel } from "./object-list-panel";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { PasswordField } from "../../shared/ui/password-field";
import { Select } from "../../shared/ui/select/select";
import { ModelIcon } from "../../shared/ui/model-icon";
import { JsonCodeEditor } from "../../shared/ui/code-editor/json-code-editor";
import { useI18n } from "../i18n/use-i18n";
import { KeyValueEditor } from "./key-value-editor";

type ProviderSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
  onProviderChange: (index: number, patch: Partial<ProviderConfig>) => void;
};

/**
 * 渲染供应商列表和当前供应商编辑表单。
 *
 * @param props 应用配置和更新回调
 * @returns 供应商设置区域
 */
export function ProviderSettingsSection({ config, onConfigChange, onProviderChange }: ProviderSettingsSectionProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const [selectedId, setSelectedId] = useState(config.active_provider || config.providers[0]?.id || "");
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState<Error | null>(null);
  const [remoteModels, setRemoteModels] = useState<string[]>([]);
  const [remoteMetadata, setRemoteMetadata] = useState<Record<string, {
    provider: string;
    context_chars?: number | null;
    max_output_tokens?: number | null;
    tags?: string[];
  }>>({});
  const [importOpen, setImportOpen] = useState(false);
  const [tab, setTab] = useState<"connection" | "models" | "behavior" | "advanced">("connection");
  const selectedIndex = Math.max(0, config.providers.findIndex((provider) => provider.id === selectedId));
  const provider = config.providers[selectedIndex];

  useEffect(() => {
    if (!config.providers.some((item) => item.id === selectedId)) {
      setSelectedId(config.active_provider || config.providers[0]?.id || "");
    }
  }, [config.active_provider, config.providers, selectedId]);

  /** 新增一项 OpenAI 兼容供应商草稿。 */
  const addProvider = () => {
    let suffix = 1;
    let id = "provider";
    while (config.providers.some((item) => item.id === id)) {
      suffix += 1;
      id = `provider-${suffix}`;
    }
    const next: ProviderConfig = {
      id,
      display_name: t("New provider", "新供应商"),
      base_url: "https://api.example.com/v1",
      protocol: "auto",
      api_key: "",
      models: [],
      default_model: "",
      thinking_level: "auto",
      thinking_format: "auto",
      client_style: "auto",
      extra_headers: {},
      extra_body: ""
    };
    onConfigChange({ ...config, providers: [...config.providers, next] });
    setSelectedId(id);
    setFetchError(null);
  };

  /** 获取当前供应商远端模型并打开导入弹层。 */
  const fetchModels = async () => {
    if (!provider) return;
    setFetching(true);
    setFetchError(null);
    try {
      const response = await api.providers.models(provider);
      setRemoteModels(response.models);
      setRemoteMetadata(response.metadata);
      setImportOpen(true);
    } catch (error) {
      setFetchError(toDisplayError(error, "Failed to fetch models", "获取模型失败"));
    } finally {
      setFetching(false);
    }
  };

  /** 将勾选的远端模型合并到当前供应商。 */
  const importModels = (models: string[]) => {
    const nextModels = [...(provider.models ?? [])];
    for (const model of models) if (!nextModels.includes(model)) nextModels.push(model);
    const modelMetadata = { ...(provider.model_metadata ?? {}) };
    for (const model of models) {
      const metadata = remoteMetadata[model];
      if (!metadata?.context_chars && !metadata?.max_output_tokens && !metadata?.tags?.length) continue;
      const current = modelMetadata[model] ?? {};
      modelMetadata[model] = {
        ...current,
        ...(!current.context_chars && metadata.context_chars ? { context_chars: metadata.context_chars } : {}),
        ...(!current.max_output_tokens && metadata.max_output_tokens ? { max_output_tokens: metadata.max_output_tokens } : {}),
        ...(metadata.tags?.length
          ? { tags: Array.from(new Set([...(current.tags ?? []), ...metadata.tags])) }
          : {})
      };
    }
    onProviderChange(selectedIndex, {
      models: nextModels,
      model_metadata: modelMetadata,
      default_model: provider.default_model || nextModels[0] || ""
    });
    setImportOpen(false);
    setTab("models");
  };

  /** 删除当前供应商并选择剩余首项。 */
  const deleteProvider = async () => {
    if (!provider) return;
    const confirmed = await confirm({
      title: t("Delete provider", "删除供应商"),
      description: t(`Delete “${provider.display_name || provider.id}” and all of its model configuration.`, `将删除“${provider.display_name || provider.id}”及其全部模型配置。`),
      confirmLabel: t("Delete provider", "删除供应商"),
      danger: true
    });
    if (!confirmed) return;
    const providers = config.providers.filter((_, index) => index !== selectedIndex);
    const activeProvider = config.active_provider === provider.id ? providers[0]?.id ?? "" : config.active_provider;
    onConfigChange({ ...config, providers, active_provider: activeProvider });
    setSelectedId(activeProvider || providers[0]?.id || "");
  };

  if (!provider) {
    return <div className="settings-empty"><button type="button" className="settings-secondary" onClick={addProvider}><Plus size={14} />{t("Add provider", "新增供应商")}</button></div>;
  }

  const models = provider.models ?? [];
  // 1. 默认模型下拉选项来自已配置模型，历史值不在列表时保留为可选项
  const defaultModelOptions = (provider.default_model && !models.includes(provider.default_model)
    ? [provider.default_model, ...models]
    : models
  ).map((model) => ({ value: model, label: model, icon: createElement(ModelIcon, { model, size: 14 }) }));
  const emptyModelOptions = [{ value: "", label: t("Add models on the Models tab first", "先在模型页签添加模型") }];
  const protocolOptions = [
    { value: "auto", label: t("Auto detect", "自动检测") },
    { value: "openai-chat", label: "OpenAI Chat Completions" },
    { value: "openai-responses", label: "OpenAI Responses" },
    { value: "anthropic", label: "Anthropic Messages" }
  ];
  const thinkingFormatOptions = [
    { value: "auto", label: t("Automatic", "自动") },
    { value: "reasoning_content", label: "reasoning_content" },
    { value: "reasoning", label: "reasoning" },
    { value: "thinking", label: "thinking" }
  ];

  return (
    <div className="settings-objects-layout">
      <ObjectListPanel
        title={t("Providers", "供应商")}
        items={config.providers.map((item) => ({
          id: item.id,
          name: item.display_name || item.id,
          meta: item.default_model || item.models?.[0] || t("No model configured", "未配置模型"),
          icon: <Cpu size={14} />,
          marked: item.id === config.active_provider
        }))}
        selectedId={selectedId}
        searchPlaceholder={t("Search providers", "搜索供应商")}
        addLabel={t("Add provider", "新增供应商")}
        onSelect={(id) => { setSelectedId(id); setFetchError(null); }}
        onAdd={addProvider}
      />
      <section className="settings-editor">
        <EditorHeader
          kicker={t("Model provider", "模型供应商")}
          title={provider.display_name || provider.id}
          description={t("Configure the endpoint, credentials, and models available from this provider.", "配置接口、凭据和当前供应商可用的模型。")}
          actions={<>
            <button type="button" className="settings-secondary" onClick={() => void fetchModels()} disabled={fetching || !provider.base_url.trim()}><RefreshCw size={14} className={fetching ? "spin" : ""} />{fetching ? t("Fetching", "正在获取") : t("Import models", "导入模型")}</button>
            <button type="button" className={provider.id === config.active_provider ? "settings-secondary active" : "settings-secondary"} onClick={() => onConfigChange({ ...config, active_provider: provider.id })} disabled={provider.id === config.active_provider}><Check size={14} />{provider.id === config.active_provider ? t("Current provider", "当前供应商") : t("Set as current", "设为当前")}</button>
            <button type="button" className="settings-danger" onClick={() => void deleteProvider()}><Trash2 size={14} />{t("Delete provider", "删除供应商")}</button>
          </>}
        />
        {fetchError && <div className="settings-inline-error">{fetchError.message}</div>}
        <nav className="settings-tabs" aria-label={t("Provider configuration categories", "供应商配置分类")}>
          <button type="button" className={tab === "connection" ? "active" : ""} onClick={() => setTab("connection")}>{t("Connection", "连接")}</button>
          <button type="button" className={tab === "models" ? "active" : ""} onClick={() => setTab("models")}>{t("Models", "模型")}</button>
          <button type="button" className={tab === "behavior" ? "active" : ""} onClick={() => setTab("behavior")}>{t("Behavior", "行为")}</button>
          <button type="button" className={tab === "advanced" ? "active" : ""} onClick={() => setTab("advanced")}>{t("Advanced", "高级")}</button>
        </nav>
        {tab === "connection" && <div className="settings-form-grid">
          <label className="settings-field"><span>{t("Provider ID", "供应商 ID")}</span><input value={provider.id} onChange={(event) => { setSelectedId(event.target.value); onProviderChange(selectedIndex, { id: event.target.value }); }} /><small>{t("Stable identifier in the configuration file", "配置文件中的稳定标识")}</small></label>
          <label className="settings-field"><span>{t("Display name", "显示名称")}</span><input value={provider.display_name} onChange={(event) => onProviderChange(selectedIndex, { display_name: event.target.value })} /><small>{t("Used in model menus and status displays", "用于模型菜单和状态展示")}</small></label>
          <label className="settings-field full"><span>{t("API address", "API 地址")}</span><input value={provider.base_url} onChange={(event) => onProviderChange(selectedIndex, { base_url: event.target.value })} spellCheck={false} /><small>{t("Base URL of the compatible API; the server accesses it when fetching models", "兼容接口的基础地址，获取模型时由服务端访问")}</small></label>
          <div className="settings-field"><span>{t("Protocol", "协议")}</span><Select value={provider.protocol ?? "auto"} options={protocolOptions} onChange={(value) => onProviderChange(selectedIndex, { protocol: value })} ariaLabel={t("Provider protocol", "供应商协议")} /><small>{t("The protocol determines request and reasoning parameter formats", "协议决定请求和思考参数格式")}</small></div>
          <div className="settings-field"><span>{t("Default model", "默认模型")}</span>
            {models.length > 0
              ? <Select value={provider.default_model ?? ""} options={defaultModelOptions} onChange={(value) => onProviderChange(selectedIndex, { default_model: value })} ariaLabel={t("Default model", "默认模型")} />
              : <Select value="" options={emptyModelOptions} disabled onChange={() => undefined} ariaLabel={t("Default model", "默认模型")} />}
            <small>{models.length > 0 ? t("Used when no model is selected manually", "未手动切换时使用") : t("Add models on the Models tab first", "先在模型页签添加模型")}</small>
          </div>
          <div className="settings-field full"><span>API Key</span><PasswordField value={provider.api_key ?? ""} onChange={(value) => onProviderChange(selectedIndex, { api_key: value })} /><small>{t("Environment variables can be referenced with `$env:VARIABLE_NAME`", "支持使用 `$env:VARIABLE_NAME` 引用环境变量")}</small></div>
        </div>}
        {tab === "behavior" && <div className="settings-form-grid">
          <label className="settings-field"><span>{t("Request timeout", "请求超时")}</span><input type="number" min="1" value={provider.timeout_seconds ?? 120} onChange={(event) => onProviderChange(selectedIndex, { timeout_seconds: Number(event.target.value) })} /><small>{t("Seconds", "单位为秒")}</small></label>
          <label className="settings-field"><span>Temperature</span><input type="number" min="0" max="2" step="0.1" value={provider.temperature ?? 0.7} onChange={(event) => onProviderChange(selectedIndex, { temperature: Number(event.target.value) })} /><small>{t("Model sampling temperature", "模型采样温度")}</small></label>
          <div className="settings-field"><span>{t("Thinking level", "思考等级")}</span><Select value={provider.thinking_level ?? "auto"} options={THINKING_OPTIONS} onChange={(value) => onProviderChange(selectedIndex, { thinking_level: value })} ariaLabel={t("Thinking level", "思考等级")} /><small>{t("Default reasoning intensity for the provider", "供应商默认推理强度")}</small></div>
          <div className="settings-field"><span>{t("Thinking format", "思考格式")}</span><Select value={provider.thinking_format ?? "auto"} options={thinkingFormatOptions} onChange={(value) => onProviderChange(selectedIndex, { thinking_format: value })} ariaLabel={t("Thinking format", "思考格式")} /><small>{t("Reasoning field in the response", "响应中的思考字段")}</small></div>
          <label className="settings-field"><span>Anthropic max_tokens</span><input type="number" min="1" value={provider.anthropic_max_tokens ?? 8192} onChange={(event) => onProviderChange(selectedIndex, { anthropic_max_tokens: Number(event.target.value) })} /><small>{t("Used by Anthropic Messages only", "仅 Anthropic Messages 使用")}</small></label>
        </div>}
        {tab === "models" && <ModelMetadataEditor provider={provider} onChange={(patch) => onProviderChange(selectedIndex, patch)} />}
        {tab === "advanced" && <div className="settings-form-grid">
          <div className="settings-field">
            <span>{t("Client style", "客户端模拟")}</span>
            <Select
              value={provider.client_style ?? "auto"}
              options={[
                { value: "auto", label: t("Auto", "自动") },
                { value: "default", label: t("Default", "默认") },
                { value: "codex", label: "Codex CLI" },
              ]}
              onChange={(value) => onProviderChange(selectedIndex, { client_style: value })}
              ariaLabel={t("Client style", "客户端模拟")}
            />
            <small>{t("Codex simulates codex_cli_rs headers and Responses body for compatible gateways", "Codex 会模拟 codex_cli_rs 请求头与 Responses 请求体")}</small>
          </div>
          <div className="settings-field full">
            <span>{t("Extra headers", "自定义请求头")}</span>
            <KeyValueEditor
              value={provider.extra_headers ?? {}}
              onChange={(extra_headers) => onProviderChange(selectedIndex, { extra_headers })}
            />
            <small>{t("Merged into each model request; Authorization is not overridden", "合并到每次模型请求，不覆盖 Authorization")}</small>
          </div>
          <div className="settings-json-field full">
            <div>
              <span>{t("Custom body JSON", "自定义 body JSON")}</span>
              <small>{t("The object is merged into each model request; explicit fields take precedence", "对象会合并到每次模型请求，显式配置字段优先")}</small>
            </div>
            <JsonCodeEditor
              value={provider.extra_body || "{}"}
              onChange={(value) => onProviderChange(selectedIndex, { extra_body: value === "{}" ? "" : value })}
              height={280}
              ariaLabel={t("Provider custom body JSON", "供应商自定义 body JSON")}
            />
          </div>
        </div>}
      </section>
      <ModelImportDialog open={importOpen} models={remoteModels} existingModels={models} onClose={() => setImportOpen(false)} onImport={importModels} />
    </div>
  );
}

const THINKING_OPTIONS = [
  { value: "auto", label: "auto" },
  { value: "max", label: "max" },
  { value: "xhigh", label: "xhigh" },
  { value: "high", label: "high" },
  { value: "medium", label: "medium" },
  { value: "low", label: "low" },
  { value: "none", label: "none" }
];
