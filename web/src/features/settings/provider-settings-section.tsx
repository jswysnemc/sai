import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { AppConfig, ProviderApiKey, ProviderConfig } from "../../api/contracts";
import { EditorHeader } from "./editor-layout";
import { ModelImportDialog } from "./model-import-dialog";
import { ObjectListPanel } from "./object-list-panel";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Toast, useToast } from "../../shared/ui/notify/notify";
import { ModelIcon } from "../../shared/ui/model-icon";
import { SkeletonText } from "../../shared/ui/skeleton/skeleton";
import { useI18n } from "../i18n/use-i18n";
import { clearNewSessionModelReference } from "../sessions/new-session-preferences";
import { useSelectedFallback } from "./controls/use-selected-fallback";
import { findProviderIndex, nextProviderId, providerIdConflict } from "./model/provider-selection";
import { formatTemperature, parseTemperature } from "./model/temperature-field";
import { providerIdFollowsName, suggestedProviderId } from "./model/provider-id-sync";
import {
  isProviderEnabled,
  nextActiveProvider,
  partitionByEnablement
} from "./model/provider-enablement";
import { ProviderHeaderActions } from "./providers/provider-header-actions";
import { ProviderConnectionTab, buildDefaultModelOptions } from "./providers/provider-connection-tab";
import { ProviderModelsTab } from "./providers/provider-models-tab";
import { ProviderBehaviorTab } from "./providers/provider-behavior-tab";
import { ProviderAdvancedTab } from "./providers/provider-advanced-tab";

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
 * 壳持有共享状态（选中项、远端模型目录、导入弹层），四个页签的表单
 * 拆分在 providers/ 目录下，每个文件只负责一个页签。
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
  const { notice, showToast, dismissToast } = useToast();
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

  /**
   * 丢弃上一个供应商拉取到的远端模型目录。
   *
   * 目录里的供应商品牌会画在默认模型下拉里，残留会让新供应商的模型
   * 顶着上一个供应商的图标。
   *
   * @returns 无返回值
   */
  const clearRemoteModels = () => {
    setRemoteModels([]);
    setRemoteMetadata({});
    setImportOpen(false);
  };

  /** 供应商 ID 的输入草稿；为 null 表示未在编辑 */
  const [idDraft, setIdDraft] = useState<string | null>(null);
  const [idError, setIdError] = useState("");
  /** 标识是否仍随显示名称同步 */
  const [idFollowsName, setIdFollowsName] = useState(true);
  const [temperatureDraft, setTemperatureDraft] = useState("");
  const [temperatureError, setTemperatureError] = useState("");
  // 找不到选中项时保持 -1：回落到 0 会让后续按索引的写入打到别的供应商上
  const selectedIndex = findProviderIndex(config.providers, selectedId);
  const provider = selectedIndex >= 0 ? config.providers[selectedIndex] : undefined;
  const providerKeys = provider ? normalizedProviderApiKeys(provider) : [];
  const selectedProviderKey = provider?.api_key_selected ?? providerKeys[0]?.id;

  // 回落到列表首项而非 active_provider：后者表示"正在使用哪个"，
  // 与"正在编辑哪个"无关，用它回落会在改名中间态把编辑区弹回旧供应商
  useSelectedFallback(selectedId, config.providers.map((item) => item.id), setSelectedId);

  useEffect(() => {
    if (!provider) return;
    setIdFollowsName(providerIdFollowsName(provider.id, provider.display_name));
    setTemperatureDraft(formatTemperature(provider.temperature));
    setTemperatureError("");
    clearRemoteModels();
  }, [provider?.id]);

  /** 新增一项 OpenAI 兼容供应商草稿。 */
  const addProvider = () => {
    const id = nextProviderId(config.providers);
    const next: ProviderConfig = {
      id,
      display_name: t("New provider", "新供应商"),
      base_url: "https://api.example.com/v1",
      protocol: "auto",
      api_key: "",
      api_keys: [{ id: "key-1", api_key: "", label: "" }],
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
    setIdFollowsName(true);
    setTemperatureDraft("");
    setTemperatureError("");
    setFetchError(null);
    setSecretError(null);
    clearRemoteModels();
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
    if (trimmed === provider.id) {
      setIdFollowsName(providerIdFollowsName(trimmed, provider.display_name));
      return;
    }
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
    setIdFollowsName(providerIdFollowsName(trimmed, provider.display_name));
    // 先更新选中项再改写配置，避免中间渲染里选中项指向已不存在的 ID
    setSelectedId(trimmed);
    onProviderChange(selectedIndex, { id: trimmed });
  };

  /**
   * 【设置】【供应商名称】更新显示名称，并在标识仍跟随名称时同步 ID。
   *
   * @param displayName 新的显示名称
   * @returns 无返回值
   */
  const updateDisplayName = (displayName: string) => {
    if (!provider || selectedIndex < 0) return;
    if (!idFollowsName) {
      onProviderChange(selectedIndex, { display_name: displayName });
      return;
    }
    const suggested = suggestedProviderId(displayName, config.providers, selectedIndex);
    if (!suggested.id || suggested.id === provider.id) {
      setIdError("");
      onProviderChange(selectedIndex, { display_name: displayName });
      return;
    }
    if (suggested.conflict) {
      setIdError(t("This name matches another provider ID. Edit the ID.", "该名称与现有供应商 ID 冲突，请编辑 ID"));
      onProviderChange(selectedIndex, { display_name: displayName });
      return;
    }
    setIdError("");
    setIdDraft(null);
    setSelectedId(suggested.id);
    onProviderChange(selectedIndex, { display_name: displayName, id: suggested.id });
  };

  /**
   * 【设置】【温度】提交温度输入。空值表示请求里不带该参数。
   *
   * @param raw 输入框文本
   * @returns 无返回值
   */
  const commitTemperature = (raw: string) => {
    if (!provider || selectedIndex < 0) return;
    const parsed = parseTemperature(raw);
    if (!parsed.ok) {
      setTemperatureError(t("Temperature must be between 0 and 2", "温度必须在 0 到 2 之间"));
      setTemperatureDraft(formatTemperature(provider.temperature));
      return;
    }
    setTemperatureError("");
    setTemperatureDraft(formatTemperature(parsed.value));
    onProviderChange(selectedIndex, { temperature: parsed.value });
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
   * 服务端也会直接拒绝解析。转移是静默发生的，因此先让用户确认。
   *
   * @param enabled 目标启用状态
   * @returns 确认流程完成后返回
   */
  const toggleProviderEnabled = async (enabled: boolean) => {
    if (!provider || selectedIndex < 0) return;
    const providers = config.providers.map((item, index) => (
      index === selectedIndex ? { ...item, enabled } : item
    ));
    const nextActive = nextActiveProvider(providers, config.active_provider);
    const name = provider.display_name || provider.id;
    if (!enabled && provider.id === config.active_provider) {
      const target = providers.find((item) => item.id === nextActive);
      const targetName = target ? target.display_name || target.id : "";
      const confirmed = await confirm({
        title: t("Disable the current provider", "停用当前供应商"),
        description: targetName
          ? t(
            `“${name}” is the current provider. Disabling it switches the current provider to “${targetName}”.`,
            `“${name}”是当前正在使用的供应商，停用后当前供应商会切换为“${targetName}”。`
          )
          : t(
            `“${name}” is the current provider. Disabling it leaves no provider in use.`,
            `“${name}”是当前正在使用的供应商，停用后将没有正在使用的供应商。`
          ),
        confirmLabel: t("Disable provider", "停用供应商"),
        danger: true
      });
      if (!confirmed) return;
    }
    onConfigChange({ ...config, providers, active_provider: nextActive });
  };

  /**
   * 【设置】【设为当前】切换当前供应商，并提示切换结果。
   *
   * @returns 无返回值
   */
  const setAsCurrentProvider = () => {
    if (!provider) return;
    const name = provider.display_name || provider.id;
    onConfigChange({ ...config, active_provider: provider.id });
    showToast(t(`“${name}” is now the current provider.`, `已将“${name}”设为当前供应商。`));
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
    return (
      <div className="settings-empty">
        <div className="settings-empty-copy">
          <strong>{t("No provider configured", "还没有配置供应商")}</strong>
          <p>
            {t(
              "Add a provider to connect a model API, then fill in its endpoint, credentials, and models.",
              "新增一个供应商即可接入模型接口，随后填写接口地址、凭据和可用模型。"
            )}
          </p>
        </div>
        <button type="button" className="settings-secondary" onClick={addProvider}><Plus size={14} />{t("Add provider", "新增供应商")}</button>
      </div>
    );
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
  // 没有 API 地址时服务端无从拉取模型列表，禁用原因挂在按钮的 tooltip 上
  const importBlockedReason = provider.base_url.trim()
    ? ""
    : t(
      "Fill in the API address first; the server fetches the model list from it.",
      "请先填写 API 地址，模型列表由服务端从该地址获取。"
    );
  const defaultModelOptions = buildDefaultModelOptions(models, provider.default_model, remoteMetadata);
  const activeModel = provider.default_model ?? "";

  /**
   * 【设置】【DeepSeek 锚定】切换当前默认模型的轨迹锚定开关。
   *
   * @param enabled 目标状态
   * @returns 无返回值
   */
  const setDeepseekAnchorEnabled = (enabled: boolean) => {
    if (!activeModel) return;
    const activeModelMetadata = provider.model_metadata?.[activeModel] ?? {};
    const nextMetadata = { ...(provider.model_metadata ?? {}) };
    if (enabled) {
      nextMetadata[activeModel] = {
        ...activeModelMetadata,
        deepseek_anchor_mode: "anchored_standard"
      };
    } else {
      const nextModelMetadata = { ...activeModelMetadata };
      delete nextModelMetadata.deepseek_anchor_mode;
      if (Object.keys(nextModelMetadata).length === 0) delete nextMetadata[activeModel];
      else nextMetadata[activeModel] = nextModelMetadata;
    }
    onProviderChange(selectedIndex, { model_metadata: nextMetadata });
  };

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
        onSelect={(id) => {
          setSelectedId(id);
          setIdDraft(null);
          setIdError("");
          setFetchError(null);
          setSecretError(null);
          clearRemoteModels();
        }}
        onAdd={addProvider}
      />
      <section className="settings-editor">
        <Toast notice={notice} onDismiss={dismissToast} />
        <EditorHeader
          kicker={t("Model provider", "模型供应商")}
          title={provider.display_name || provider.id}
          description={t("Configure the endpoint, credentials, and models available from this provider.", "配置接口、凭据和当前供应商可用的模型。")}
          actions={
            <ProviderHeaderActions
              config={config}
              provider={provider}
              enabled={isProviderEnabled(provider)}
              isCurrent={provider.id === config.active_provider}
              fetching={fetching}
              importBlockedReason={importBlockedReason}
              onToggleEnabled={(enabled) => void toggleProviderEnabled(enabled)}
              onFetchModels={() => void fetchModels()}
              onSetCurrent={setAsCurrentProvider}
              onDelete={() => void deleteProvider()}
            />
          }
        />
        {fetchError && <div className="settings-inline-error">{fetchError.message}</div>}
        {secretError && <div className="settings-inline-error">{secretError.message}</div>}
        {fetching && <div className="provider-editor-loading">
          <SkeletonText lines={6} label={t("Fetching the model list", "正在获取模型列表")} />
        </div>}
        {!fetching && tab === "connection" && (
          <ProviderConnectionTab
            provider={provider}
            providerIndex={selectedIndex}
            providerKeys={providerKeys}
            selectedProviderKey={selectedProviderKey}
            secretSentinel={secretSentinel}
            idDraft={idDraft}
            idError={idError}
            defaultModelOptions={defaultModelOptions}
            remoteMetadata={remoteMetadata}
            onIdDraftChange={setIdDraft}
            onCommitId={commitProviderId}
            onIdEscape={() => {
              setIdDraft(null);
              setIdError("");
            }}
            onDisplayNameChange={updateDisplayName}
            onPatch={(patch) => onProviderChange(selectedIndex, patch)}
            onRevealKey={revealProviderApiKey}
            onKeysChange={(patch) => {
              setSecretError(null);
              onProviderChange(selectedIndex, { ...patch, api_key: "" });
            }}
          />
        )}
        {!fetching && tab === "models" && (
          <ProviderModelsTab provider={provider} onChange={updateModelConfiguration} />
        )}
        {!fetching && tab === "behavior" && (
          <ProviderBehaviorTab
            provider={provider}
            temperatureDraft={temperatureDraft}
            temperatureError={temperatureError}
            onTemperatureDraftChange={(value) => {
              setTemperatureDraft(value);
              setTemperatureError("");
            }}
            onCommitTemperature={commitTemperature}
            onPatch={(patch) => onProviderChange(selectedIndex, patch)}
            onDeepseekAnchorChange={setDeepseekAnchorEnabled}
          />
        )}
        {!fetching && tab === "advanced" && (
          <ProviderAdvancedTab
            provider={provider}
            onPatch={(patch) => onProviderChange(selectedIndex, patch)}
          />
        )}
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
