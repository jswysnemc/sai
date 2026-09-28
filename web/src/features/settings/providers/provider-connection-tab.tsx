import { createElement, useId } from "react";
import type { ProviderApiKey, ProviderConfig } from "../../../api/contracts";
import { ModelIcon } from "../../../shared/ui/model-icon";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkSelect, SkTextInput } from "../kit";
import { fieldAnchorId } from "../search/field-anchor";
import { ProviderApiKeysField } from "../provider-api-keys-field";
import { ProviderConnectionTest } from "../model/provider-connection-test";
import { protocolOptions } from "./provider-options";

type ProviderConnectionTabProps = {
  provider: ProviderConfig;
  providerIndex: number;
  providerKeys: ProviderApiKey[];
  selectedProviderKey: string | undefined;
  secretSentinel: string;
  idDraft: string | null;
  idError: string;
  defaultModelOptions: Array<{ value: string; label: string; icon: React.ReactNode }>;
  remoteMetadata: Record<string, { provider?: string }>;
  onIdDraftChange: (value: string) => void;
  onCommitId: (value: string) => void;
  onIdEscape: () => void;
  onDisplayNameChange: (value: string) => void;
  onPatch: (patch: Partial<ProviderConfig>) => void;
  onRevealKey: (keyId: string) => Promise<string>;
  onKeysChange: (patch: Partial<ProviderConfig>) => void;
};

/**
 * 【供应商设置】【连接】编辑地址、模型、凭据与身份并保留草稿探测入口。
 * @param props 供应商、密钥、标识草稿与更新回调
 * @returns 连接设置页签
 */
export function ProviderConnectionTab({ provider, providerKeys, selectedProviderKey, secretSentinel, idDraft, idError, defaultModelOptions, onIdDraftChange, onCommitId, onIdEscape, onDisplayNameChange, onPatch, onRevealKey, onKeysChange }: ProviderConnectionTabProps) {
  const { t } = useI18n();
  const probeFormId = useId();
  return <>
    <SettingsPanel title={t("Endpoint", "接入点")} description={t("Where requests go and which protocol they use.", "配置请求地址、默认模型与接口协议。")}>
      <FieldGrid>
        <SettingsField label={t("API address", "API 地址")} anchor="providers.connection.base_url" configKey="providers.base_url" span="full" hint={t("Base URL of the compatible API, including its version path.", "兼容接口的基础地址，包含版本路径。")}>
          <SkTextInput form={probeFormId} title={t("Press Enter to test connection", "按回车测试连接")} mono value={provider.base_url} onChange={(value) => onPatch({ base_url: value })} />
        </SettingsField>
        <SettingsField label={t("Default model", "默认模型")} anchor="providers.connection.default_model" configKey="providers.default_model" hint={t("Used when no model is selected manually.", "未手动选择模型时使用。")}>
          <SkSelect value={provider.default_model ?? ""} options={defaultModelOptions.length ? defaultModelOptions : [{ value: "", label: t("Add models on the Models tab first", "先在模型页签添加模型") }]} disabled={!defaultModelOptions.length} onChange={(value) => onPatch({ default_model: value })} />
        </SettingsField>
        <SettingsField label={t("Protocol", "协议")} anchor="providers.connection.protocol" configKey="providers.protocol" hint={t("Determines request and reasoning parameter formats.", "决定请求与思考参数的格式。")}>
          <SkSelect value={provider.protocol ?? "auto"} options={protocolOptions()} onChange={(value) => onPatch({ protocol: value })} />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
    <SettingsPanel title={t("Credentials", "凭据")} description={t("Use a selected key or rotate across multiple keys. Environment references use $env:VARIABLE_NAME.", "固定使用所选密钥或在多个密钥间轮换。环境变量使用 $env:VARIABLE_NAME 引用。")} id={fieldAnchorId("providers.connection.api_keys")}>
      <ProviderApiKeysField key={provider.id} providerId={provider.id} keys={providerKeys} selected={selectedProviderKey} balance={provider.api_key_balance === true} secretSentinel={secretSentinel} onRevealKey={onRevealKey} onChange={onKeysChange} />
    </SettingsPanel>
    <SettingsPanel title={t("Connectivity", "连通性")} description={t("Test normal responses or tool calls using the selected key.", "使用所选密钥测试普通响应或工具调用。")}>
      <ProviderConnectionTest formId={probeFormId} key={`${provider.id}:${provider.default_model ?? ""}:${selectedProviderKey ?? ""}`} provider={provider} model={provider.default_model || undefined} selectedKeyId={selectedProviderKey} />
    </SettingsPanel>
    <SettingsPanel title={t("Identity", "身份")}>
      <FieldGrid>
        <SettingsField label={t("Display name", "显示名称")} anchor="providers.connection.display_name" configKey="providers.display_name" hint={t("The ID follows this name until you edit the ID manually.", "手动修改 ID 前，标识会随显示名称更新。")}>
          <SkTextInput value={provider.display_name} onChange={onDisplayNameChange} />
        </SettingsField>
        <SettingsField label={t("Provider ID", "供应商 ID")} anchor="providers.connection.id" configKey="providers.id" error={idError || undefined} hint={t("Stable identifier used by configuration references.", "供配置引用使用的稳定标识。")}>
          <SkTextInput value={idDraft ?? provider.id} onChange={onIdDraftChange} onBlur={(event) => onCommitId(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); if (event.key === "Escape") onIdEscape(); }} />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
  </>;
}

/** 新建供应商时填入的占位地址。 */
export const PLACEHOLDER_BASE_URL = "https://api.example.com/v1";

/**
 * 构造默认模型下拉选项；历史值不在模型列表时保留为可选项。
 *
 * @param models 已配置模型
 * @param defaultModel 当前默认模型
 * @param remoteMetadata 远端模型目录，用于品牌图标
 * @returns 下拉选项
 */
export function buildDefaultModelOptions(
  models: string[],
  defaultModel: string | undefined,
  remoteMetadata: Record<string, { provider?: string }>
): Array<{ value: string; label: string; icon: React.ReactNode }> {
  const list = defaultModel && !models.includes(defaultModel)
    ? [defaultModel, ...models]
    : models;
  return list.map((model) => ({
    value: model,
    label: model,
    icon: createElement(ModelIcon, {
      model,
      provider: remoteMetadata[model]?.provider,
      size: 14
    })
  }));
}
