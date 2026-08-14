import { Check, Plus, RefreshCw, Trash2 } from "lucide-react";
import { createElement, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { AppConfig, ProviderApiKey, ProviderConfig } from "../../api/contracts";
import { EditorHeader } from "./editor-layout";
import { ProviderConnectionTest } from "./model/provider-connection-test";
import { ModelMetadataEditor } from "./model-metadata-editor";
import { ModelImportDialog } from "./model-import-dialog";
import { ObjectListPanel } from "./object-list-panel";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Select } from "../../shared/ui/select/select";
import { ModelIcon } from "../../shared/ui/model-icon";
import { JsonCodeEditor } from "../../shared/ui/code-editor/json-code-editor";
import { useI18n } from "../i18n/use-i18n";
import { clearNewSessionModelReference } from "../sessions/new-session-preferences";
import { KeyValueEditor } from "./key-value-editor";
import { ProviderApiKeysField } from "./provider-api-keys-field";
import { useSelectedFallback } from "./controls/use-selected-fallback";
import { findProviderIndex, nextProviderId, providerIdConflict } from "./model/provider-selection";
import {
  isProviderEnabled,
  nextActiveProvider,
  partitionByEnablement
} from "./model/provider-enablement";

type ProviderSettingsSectionProps = {
  config: AppConfig;
  /** 当前子页：connection / models / behavior / advanced */
  subview?: string;
  secretSentinel: string;
  onConfigChange: (config: AppConfig) => void;
  onProviderChange: (index: number, patch: Partial<ProviderConfig>) => void;
};

/**
 * 将旧版单密钥配置转换为统一的密钥列表展示。
 *
 * @param provider 当前供应商配置
 * @returns 用于编辑器展示的密钥列表
 */
function normalizedProviderApiKeys(provider: ProviderConfig): ProviderApiKey[] {
  if (provider.api_keys && provider.api_keys.length > 0) return provider.api_keys;
  if (!provider.api_key?.trim()) return [];
  return [{ id: "key-1", api_key: provider.api_key, label: "" }];
}

/**
 * 渲染供应商列表和当前供应商编辑表单。
 *
 * @param props 应用配置和更新回调
 * @returns 供应商设置区域
 */
export function ProviderSettingsSection({
  config,
  subview,
  secretSentinel,
  onConfigChange,
  onProviderChange
}: ProviderSettingsSectionProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const navigate = useNavigate();
  // 子页由路由解析保证合法，此处仅作类型收窄的回落
  const tab = (subview ?? "connection") as "connection" | "models" | "behavior" | "advanced";
  const [selectedId, setSelectedId] = useState(config.active_provider || config.providers[0]?.id || "");
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState<Error | null>(null);
  const [secretError, setSecretError] = useState<Error | null>(null);
  const [remoteModels, setRemoteModels] = useState<string[]>([]);
  const [remoteMetadata, setRemoteMetadata] = useState<Record<string, {
    provider: string;
    context_chars?: number | null;
    max_output_tokens?: number | null;
    tags?: string[];
    thinking_levels?: string[];
  }>>({});
  const [importOpen, setImportOpen] = useState(false);
  /** 供应商 ID 的输入草稿；为 null 表示未在编辑 */
  const [idDraft, setIdDraft] = useState<string | null>(null);
  const [idError, setIdError] = useState("");
  // 找不到选中项时保持 -1：回落到 0 会让后续按索引的写入打到别的供应商上
  const selectedIndex = findProviderIndex(config.providers, selectedId);
  const provider = selectedIndex >= 0 ? config.providers[selectedIndex] : undefined;
  const providerKeys = provider ? normalizedProviderApiKeys(provider) : [];
  const selectedProviderKey = provider?.api_key_selected ?? providerKeys[0]?.id;
  const claudeSimulation = isClaudeClientStyle(provider?.client_style);

  // 回落到列表首项而非 active_provider：后者表示"正在使用哪个"，
  // 与"正在编辑哪个"无关，用它回落会在改名中间态把编辑区弹回旧供应商
  useSelectedFallback(selectedId, config.providers.map((item) => item.id), setSelectedId);

  /** 新增一项 OpenAI 兼容供应商草稿。 */
  const addProvider = () => {
    const id = nextProviderId(config.providers);
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
      claude_1m_context: true,
      user_agent: "",
      extra_headers: {},
      extra_body: ""
    };
    onConfigChange({ ...config, providers: [...config.providers, next] });
    setSelectedId(id);
    setIdDraft(null);
    setIdError("");
    setFetchError(null);
    setSecretError(null);
  };

  /**
   * 【设置】【供应商标识】提交供应商 ID 改名。
   *
   * ID 是配置主键，边输入边提交会在中间态与其他供应商重名、并让选中项瞬时失配，
   * 因此仅在结束编辑时整体提交，重名或留空则回退为原值。
   *
   * @param nextId 目标标识
   * @returns 无返回值
   */
  const commitProviderId = (nextId: string) => {
    setIdDraft(null);
    if (!provider || selectedIndex < 0) return;
    const trimmed = nextId.trim();
    if (trimmed === provider.id) return;
    const conflict = providerIdConflict(config.providers, selectedIndex, trimmed);
    if (conflict) {
      setIdError(
        conflict === "empty"
          ? t("Provider ID cannot be empty", "供应商 ID 不能为空")
          : t("Provider ID is already used", "供应商 ID 已被占用")
      );
      return;
    }
    setIdError("");
    // 先更新选中项再改写配置，避免中间渲染里选中项指向已不存在的 ID
    setSelectedId(trimmed);
    onProviderChange(selectedIndex, { id: trimmed });
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
    // 选中项失配时不写入：按错误索引写会把模型并进别的供应商
    if (!provider || selectedIndex < 0) {
      setImportOpen(false);
      return;
    }
    const nextModels = [...(provider.models ?? [])];
    for (const model of models) if (!nextModels.includes(model)) nextModels.push(model);
    const modelMetadata = { ...(provider.model_metadata ?? {}) };
    for (const model of models) {
      const metadata = remoteMetadata[model];
      if (!metadata?.context_chars && !metadata?.max_output_tokens && !metadata?.tags?.length
        && !metadata?.thinking_levels?.length) continue;
      const current = modelMetadata[model] ?? {};
      modelMetadata[model] = {
        ...current,
        ...(!current.context_chars && metadata.context_chars ? { context_chars: metadata.context_chars } : {}),
        ...(!current.max_output_tokens && metadata.max_output_tokens ? { max_output_tokens: metadata.max_output_tokens } : {}),
        ...(metadata.tags?.length
          ? { tags: Array.from(new Set([...(current.tags ?? []), ...metadata.tags])) }
          : {}),
        // 已手工设置过的支持范围不被目录覆盖：这里正是纠正目录错误的地方
        ...(!current.thinking_levels?.length && metadata.thinking_levels?.length
          ? { thinking_levels: metadata.thinking_levels }
          : {})
      };
    }
    onProviderChange(selectedIndex, {
      models: nextModels,
      model_metadata: modelMetadata,
      default_model: provider.default_model || nextModels[0] || ""
    });
    setImportOpen(false);
    navigate("/settings/providers/models");
  };

  /**
   * 【设置】【供应商模型】更新模型目录，并在模型被移除时清理新会话引用。
   *
   * @param patch 模型目录局部更新
   * @returns 无返回值
   */
  const updateModelConfiguration = (patch: Partial<ProviderConfig>) => {
    if (!provider) return;
    const nextProvider = { ...provider, ...patch };
    const nextProviders = config.providers.map((item, index) => (
      index === selectedIndex ? nextProvider : item
    ));
    const configuredModel = config.session?.new_session_model ?? "";
    const availableModels = nextProvider.models?.length
      ? nextProvider.models
      : [nextProvider.default_model ?? ""];
    const clearsNewSessionModel = config.session?.new_session_provider_id === provider.id
      && Boolean(configuredModel)
      && !availableModels.includes(configuredModel);
    onConfigChange({
      ...config,
      providers: nextProviders,
      session: clearsNewSessionModel
        ? clearNewSessionModelReference(config.session)
        : config.session
    });
  };

  /**
   * 【设置】【供应商启用】切换启用状态，并在需要时转移当前供应商。
   *
   * 停用正在使用的供应商却不转移，会让后续请求全部落在一个已停用的配置上；
   * 服务端也会直接拒绝解析。
   *
   * @param enabled 目标启用状态
   * @returns 无返回值
   */
  const toggleProviderEnabled = (enabled: boolean) => {
    if (!provider || selectedIndex < 0) return;
    const providers = config.providers.map((item, index) => (
      index === selectedIndex ? { ...item, enabled } : item
    ));
    onConfigChange({
      ...config,
      providers,
      active_provider: nextActiveProvider(providers, config.active_provider)
    });
  };

  /**
   * 【设置】【供应商删除】删除当前供应商并选择剩余首项。
   *
   * @returns 删除流程完成后返回
   */
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
    const clearsNewSessionModel = config.session?.new_session_provider_id === provider.id;
    onConfigChange({
      ...config,
      providers,
      active_provider: activeProvider,
      session: clearsNewSessionModel
        ? clearNewSessionModelReference(config.session)
        : config.session
    });
    setSelectedId(activeProvider || providers[0]?.id || "");
  };

  if (!provider) {
    return <div className="settings-empty"><button type="button" className="settings-secondary" onClick={addProvider}><Plus size={14} />{t("Add provider", "新增供应商")}</button></div>;
  }

  /**
   * 【设置】【密钥查看】按需读取当前供应商实际使用的 API Key。
   *
   * @returns 服务端解析后的真实 API Key
   */
  const revealProviderApiKey = async (keyId: string): Promise<string> => {
    setSecretError(null);
    try {
      const response = await api.config.providerSecret(provider.id, provider.api_keys?.length ? keyId : undefined);
      return response.api_key;
    } catch (error) {
      setSecretError(toDisplayError(error, "Failed to reveal API key", "读取 API Key 失败"));
      throw error;
    }
  };

  const models = provider.models ?? [];
  // 1. 默认模型下拉选项来自已配置模型，历史值不在列表时保留为可选项
  const defaultModelOptions = (provider.default_model && !models.includes(provider.default_model)
    ? [provider.default_model, ...models]
    : models
  ).map((model) => ({
    value: model,
    label: model,
    icon: createElement(ModelIcon, {
      model,
      provider: remoteMetadata[model]?.provider,
      size: 14
    })
  }));
  const emptyModelOptions = [{ value: "", label: t("Add models on the Models tab first", "先在模型页签添加模型") }];
  const protocolOptions = [
    { value: "auto", label: t("Auto detect", "自动检测") },
    { value: "openai-chat", label: "OpenAI Chat Completions" },
    { value: "openai-responses", label: "OpenAI Responses" },
    { value: "anthropic", label: "Anthropic Messages" }
  ];
  // 取值必须与后端白名单（src/config/app.rs 的 thinking_format 校验）一致，
  // 否则保存时会被拒绝
  const thinkingFormatOptions = [
    { value: "auto", label: t("Automatic", "自动") },
    { value: "openai-chat-reasoning-effort", label: "openai-chat-reasoning-effort" },
    { value: "reasoning", label: "reasoning" },
    { value: "anthropic-thinking", label: "anthropic-thinking" },
    { value: "deepseek-thinking", label: "deepseek-thinking" },
    { value: "moonshot-thinking", label: "moonshot-thinking" },
    { value: "string", label: "string" },
    { value: "object", label: "object" },
    { value: "disabled", label: t("Disabled", "停用") }
  ];

  const providerGroups = partitionByEnablement(config.providers);
  /**
   * 把供应商配置转换为列表行。
   *
   * @param item 供应商配置
   * @returns 列表行数据
   */
  const providerListItem = (item: ProviderConfig) => ({
    id: item.id,
    name: item.display_name || item.id,
    meta: item.default_model || item.models?.[0] || t("No model configured", "未配置模型"),
    // 品牌图标按供应商与默认模型解析，拉不到时降级为模型名首字母块；
    // 所有供应商共用一个 Cpu 图标既认不出来源，也和"设置"语义混淆
    icon: <ModelIcon model={item.default_model || item.models?.[0] || item.id} provider={item.id} size={14} />,
    marked: item.id === config.active_provider,
    muted: !isProviderEnabled(item)
  });

  return (
    <div className="settings-objects-layout">
      <ObjectListPanel
        title={t("Providers", "供应商")}
        items={providerGroups.enabled.map(providerListItem)}
        collapsedItems={providerGroups.disabled.map(providerListItem)}
        collapsedTitle={t("Disabled", "已停用")}
        selectedId={selectedId}
        searchPlaceholder={t("Search providers", "搜索供应商")}
        addLabel={t("Add provider", "新增供应商")}
        onSelect={(id) => { setSelectedId(id); setIdDraft(null); setIdError(""); setFetchError(null); setSecretError(null); }}
        onAdd={addProvider}
      />
      <section className="settings-editor">
        <EditorHeader
          kicker={t("Model provider", "模型供应商")}
          title={provider.display_name || provider.id}
          description={t("Configure the endpoint, credentials, and models available from this provider.", "配置接口、凭据和当前供应商可用的模型。")}
          actions={<>
            <label className="settings-switch">
              <input
                type="checkbox"
                checked={isProviderEnabled(provider)}
                onChange={(event) => toggleProviderEnabled(event.target.checked)}
              />
              <span />
              <strong>{isProviderEnabled(provider) ? t("Enabled", "已启用") : t("Disabled", "已停用")}</strong>
            </label>
            <button type="button" className="settings-secondary" onClick={() => void fetchModels()} disabled={fetching || !provider.base_url.trim()}><RefreshCw size={14} className={fetching ? "spin" : ""} />{fetching ? t("Fetching", "正在获取") : t("Import models", "导入模型")}</button>
            <button type="button" className={provider.id === config.active_provider ? "settings-secondary active" : "settings-secondary"} onClick={() => onConfigChange({ ...config, active_provider: provider.id })} disabled={provider.id === config.active_provider || !isProviderEnabled(provider)}><Check size={14} />{provider.id === config.active_provider ? t("Current provider", "当前供应商") : t("Set as current", "设为当前")}</button>
            <button type="button" className="settings-danger" onClick={() => void deleteProvider()}><Trash2 size={14} />{t("Delete provider", "删除供应商")}</button>
          </>}
        />
        {fetchError && <div className="settings-inline-error">{fetchError.message}</div>}
        {secretError && <div className="settings-inline-error">{secretError.message}</div>}
        {tab === "connection" && <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Provider ID", "供应商 ID")}</span>
            <input
              value={idDraft ?? provider.id}
              onChange={(event) => setIdDraft(event.target.value)}
              onBlur={(event) => commitProviderId(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
                if (event.key === "Escape") {
                  setIdDraft(null);
                  setIdError("");
                }
              }}
              spellCheck={false}
            />
            <small>{idError || t("Stable identifier in the configuration file", "配置文件中的稳定标识")}</small>
          </label>
          <label className="settings-field"><span>{t("Display name", "显示名称")}</span><input value={provider.display_name} onChange={(event) => onProviderChange(selectedIndex, { display_name: event.target.value })} /><small>{t("Used in model menus and status displays", "用于模型菜单和状态展示")}</small></label>
          <label className="settings-field full"><span>{t("API address", "API 地址")}</span><input value={provider.base_url} onChange={(event) => onProviderChange(selectedIndex, { base_url: event.target.value })} spellCheck={false} /><small>{t("Base URL of the compatible API; the server accesses it when fetching models", "兼容接口的基础地址，获取模型时由服务端访问")}</small></label>
          <div className="settings-field"><span>{t("Protocol", "协议")}</span><Select value={provider.protocol ?? "auto"} options={protocolOptions} onChange={(value) => onProviderChange(selectedIndex, { protocol: value })} ariaLabel={t("Provider protocol", "供应商协议")} /><small>{t("The protocol determines request and reasoning parameter formats", "协议决定请求和思考参数格式")}</small></div>
          <div className="settings-field"><span>{t("Default model", "默认模型")}</span>
            {models.length > 0
              ? <Select value={provider.default_model ?? ""} options={defaultModelOptions} onChange={(value) => onProviderChange(selectedIndex, { default_model: value })} ariaLabel={t("Default model", "默认模型")} />
              : <Select value="" options={emptyModelOptions} disabled onChange={() => undefined} ariaLabel={t("Default model", "默认模型")} />}
            <small>{models.length > 0 ? t("Used when no model is selected manually", "未手动切换时使用") : t("Add models on the Models tab first", "先在模型页签添加模型")}</small>
          </div>
          <div className="settings-field full">
            <ProviderApiKeysField
              providerId={provider.id}
              keys={providerKeys}
              selected={selectedProviderKey}
              balance={provider.api_key_balance === true}
              secretSentinel={secretSentinel}
              onRevealKey={revealProviderApiKey}
              onChange={(patch) => {
                setSecretError(null);
                onProviderChange(selectedIndex, {
                  ...patch,
                  api_key: ""
                });
              }}
            />
            <small>{t("Use one selected key by default, or enable load balancing when multiple keys are configured. Environment variables can be referenced with `$env:VARIABLE_NAME`.", "默认使用一个选中的密钥；配置多个密钥后可以启用负载均衡。支持使用 `$env:VARIABLE_NAME` 引用环境变量。")}</small>
          </div>
          <div className="settings-field full">
            <span>{t("Connectivity", "连通性")}</span>
            <ProviderConnectionTest
              key={`${provider.id}:${provider.default_model ?? ""}:${selectedProviderKey ?? ""}`}
              provider={provider}
              model={provider.default_model || undefined}
              selectedKeyId={selectedProviderKey}
            />
            <small>{t("Run a normal model response test or a separate tool-calling test with the selected key.", "可以使用当前选中的密钥分别测试普通模型响应和工具调用。")}</small>
          </div>
        </div>}
        {tab === "behavior" && <div className="settings-form-grid">
          <label className="settings-field"><span>{t("Request timeout", "请求超时")}</span><input type="number" min="1" value={provider.timeout_seconds ?? 120} onChange={(event) => onProviderChange(selectedIndex, { timeout_seconds: Number(event.target.value) })} /><small>{t("Seconds", "单位为秒")}</small></label>
          <label className="settings-field"><span>Temperature</span><input type="number" min="0" max="2" step="0.1" value={provider.temperature ?? 0.7} onChange={(event) => onProviderChange(selectedIndex, { temperature: Number(event.target.value) })} /><small>{t("Model sampling temperature", "模型采样温度")}</small></label>
          <div className="settings-field"><span>{t("Thinking level", "思考等级")}</span><Select value={provider.thinking_level ?? "auto"} options={THINKING_OPTIONS} onChange={(value) => onProviderChange(selectedIndex, { thinking_level: value })} ariaLabel={t("Thinking level", "思考等级")} /><small>{t("Default reasoning intensity for the provider", "供应商默认推理强度")}</small></div>
          <div className="settings-field"><span>{t("Thinking format", "思考格式")}</span><Select value={provider.thinking_format ?? "auto"} options={thinkingFormatOptions} onChange={(value) => onProviderChange(selectedIndex, { thinking_format: value })} ariaLabel={t("Thinking format", "思考格式")} /><small>{t("Reasoning field in the response", "响应中的思考字段")}</small></div>
          <label className="settings-toggle-field">
            <span>
              <strong>{t("Preserve thinking", "回传历史思考")}</strong>
              <small>{t(
                "Send previous reasoning_content back in multi-turn requests; required by models such as kimi-k2.7-code.",
                "多轮请求回传历史 reasoning_content；kimi-k2.7-code 一类模型要求开启。"
              )}</small>
            </span>
            <input
              type="checkbox"
              checked={provider.preserve_thinking === true}
              onChange={(event) => onProviderChange(selectedIndex, { preserve_thinking: event.target.checked })}
            />
          </label>
          {claudeSimulation && (
            <label className="settings-field">
              <span>{t("Claude max output", "Claude 最大输出")}</span>
              <input
                type="number"
                min="1"
                value={provider.anthropic_max_tokens ?? 8192}
                onChange={(event) =>
                  onProviderChange(selectedIndex, {
                    anthropic_max_tokens: Number(event.target.value),
                  })
                }
              />
              <small>{t("Anthropic Messages max_tokens for Claude simulation", "Claude 模拟时 Anthropic Messages 的 max_tokens")}</small>
            </label>
          )}
        </div>}
        {tab === "models" && <ModelMetadataEditor provider={provider} onChange={updateModelConfiguration} />}
        {tab === "advanced" && <div className="provider-advanced-layout">
          <div className="provider-advanced-top">
            <div className="settings-field">
              <span>{t("Client style", "客户端模拟")}</span>
              <Select
                value={provider.client_style ?? "auto"}
                options={[
                  { value: "auto", label: t("Auto", "自动") },
                  { value: "default", label: t("Default", "默认") },
                  { value: "codex", label: "Codex CLI" },
                  { value: "claude", label: "Claude Code" },
                ]}
                onChange={(value) => onProviderChange(selectedIndex, { client_style: value })}
                ariaLabel={t("Client style", "客户端模拟")}
              />
              <small>{t("Codex forces Responses body and codex_cli_rs headers. Claude forces Anthropic Messages with Claude Code headers (beta, x-app, session). Use for 1M-context Claude proxies.", "Codex 强制 Responses 与 codex_cli_rs 头。Claude 强制 Anthropic Messages 与 Claude Code 头（beta、x-app、session）。适用于 1M 上下文 Claude 代理。")}</small>
            </div>
            {claudeSimulation && (
              <label className="settings-toggle-field">
                <span>
                  <strong>{t("Claude 1M context", "Claude 启用 1M 上下文")}</strong>
                  <small>{t(
                    "Attach context-1m-2025-08-07 in anthropic-beta. Enabled by default.",
                    "在 anthropic-beta 中附加 context-1m-2025-08-07，默认启用。"
                  )}</small>
                </span>
                <input
                  type="checkbox"
                  checked={provider.claude_1m_context !== false}
                  onChange={(event) => onProviderChange(selectedIndex, {
                    claude_1m_context: event.target.checked
                  })}
                />
              </label>
            )}
            <label className="settings-field">
              <span>User-Agent</span>
              <input
                value={provider.user_agent ?? ""}
                onChange={(event) => onProviderChange(selectedIndex, { user_agent: event.target.value })}
                spellCheck={false}
                placeholder={
                  provider.client_style === "codex"
                    ? "codex_cli_rs/0.144.0"
                    : claudeSimulation
                      ? "claude-cli/2.1.113 (external, cli)"
                      : "sai/0.1"
                }
              />
              <small>{t("Empty uses Codex/Claude CLI UA when Client style matches, otherwise sai/0.1. Overrides User-Agent in extra headers.", "留空时：客户端模拟为 Codex/Claude 则用对应 CLI UA，否则 sai/0.1。优先于自定义请求头中的 User-Agent。")}</small>
            </label>
          </div>
          <div className="provider-advanced-panels">
            <div className="settings-field provider-advanced-panel">
              <span>{t("Extra headers", "自定义请求头")}</span>
              <KeyValueEditor
                value={provider.extra_headers ?? {}}
                onChange={(extra_headers) => onProviderChange(selectedIndex, { extra_headers })}
              />
              <small>{t("Merged into each model request; Authorization is not overridden", "合并到每次模型请求，不覆盖 Authorization")}</small>
            </div>
            <div className="settings-json-field provider-advanced-panel">
              <div>
                <span>{t("Custom body JSON", "自定义 body JSON")}</span>
                <small>{t("The object is merged into each model request; explicit fields take precedence", "对象会合并到每次模型请求，显式配置字段优先")}</small>
              </div>
              <JsonCodeEditor
                value={provider.extra_body || "{}"}
                onChange={(value) => onProviderChange(selectedIndex, { extra_body: value === "{}" ? "" : value })}
                height={220}
                ariaLabel={t("Provider custom body JSON", "供应商自定义 body JSON")}
              />
            </div>
          </div>
        </div>}
      </section>
      <ModelImportDialog
        open={importOpen}
        models={remoteModels}
        existingModels={models}
        metadata={remoteMetadata}
        onClose={() => setImportOpen(false)}
        onImport={importModels}
      />
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

/**
 * 判断客户端模拟是否为 Claude Code。
 *
 * @param style 客户端模拟配置
 * @returns Claude 模拟时 true
 */
function isClaudeClientStyle(style?: string): boolean {
  const normalized = (style ?? "auto").trim().toLowerCase();
  return normalized === "claude" || normalized === "claude-code" || normalized === "claude_code";
}
