import { createElement } from "react";
import type { ProviderApiKey, ProviderConfig } from "../../../api/contracts";
import { ModelIcon } from "../../../shared/ui/model-icon";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
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
 * 供应商编辑器的连接页签：身份、接入点、凭据与连通性。
 *
 * 页签内部按接入流程分为两组：先填身份与地址，再配密钥并验证，
 * 与 runtime 等分区的 SettingsGroup 视觉语言一致。
 *
 * @param props 供应商状态与更新回调
 * @returns 连接页签内容
 */
export function ProviderConnectionTab({
  provider,
  providerIndex,
  providerKeys,
  selectedProviderKey,
  secretSentinel,
  idDraft,
  idError,
  defaultModelOptions,
  remoteMetadata,
  onIdDraftChange,
  onCommitId,
  onIdEscape,
  onDisplayNameChange,
  onPatch,
  onRevealKey,
  onKeysChange
}: ProviderConnectionTabProps) {
  const { t } = useI18n();
  const models = provider.models ?? [];
  const emptyModelOptions = [{ value: "", label: t("Add models on the Models tab first", "先在模型页签添加模型") }];

  return (
    <>
      <SettingsGroup
        title={t("Identity", "身份")}
        description={t(
          "Display name and the stable ID stored in the configuration file.",
          "界面显示名与配置文件中的稳定标识。"
        )}
      >
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Display name", "显示名称")}</span>
            <input
              value={provider.display_name}
              onChange={(event) => onDisplayNameChange(event.target.value)}
            />
            <small>{t("Used in model menus and status displays. The ID follows this name until you edit it.", "用于模型菜单和状态展示。未手动改 ID 时，标识会跟随名称。")}</small>
          </label>
          <label className="settings-field">
            <span>{t("Provider ID", "供应商 ID")}</span>
            <input
              value={idDraft ?? provider.id}
              onChange={(event) => onIdDraftChange(event.target.value)}
              onBlur={(event) => onCommitId(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
                if (event.key === "Escape") onIdEscape();
              }}
              spellCheck={false}
              aria-invalid={idError ? true : undefined}
            />
            <small className={idError ? "settings-field-error" : undefined}>{idError || t("Stable identifier in the configuration file", "配置文件中的稳定标识")}</small>
          </label>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Endpoint", "接入点")}
        description={t(
          "Where requests go and which protocol they speak.",
          "请求发往哪里、使用哪种协议。"
        )}
      >
        <div className="settings-form-grid">
          <label className="settings-field full">
            <span>{t("API address", "API 地址")}</span>
            <input
              value={provider.base_url}
              onChange={(event) => onPatch({ base_url: event.target.value })}
              spellCheck={false}
            />
            <small>{t("Base URL of the compatible API; the server accesses it when fetching models", "兼容接口的基础地址，获取模型时由服务端访问")}</small>
          </label>
          <div className="settings-field">
            <span>{t("Protocol", "协议")}</span>
            <Select
              value={provider.protocol ?? "auto"}
              options={protocolOptions()}
              onChange={(value) => onPatch({ protocol: value })}
              ariaLabel={t("Provider protocol", "供应商协议")}
            />
            <small>{t("The protocol determines request and reasoning parameter formats", "协议决定请求和思考参数格式")}</small>
          </div>
          <div className="settings-field">
            <span>{t("Default model", "默认模型")}</span>
            {models.length > 0
              ? <Select value={provider.default_model ?? ""} options={defaultModelOptions} onChange={(value) => onPatch({ default_model: value })} ariaLabel={t("Default model", "默认模型")} />
              : <Select value="" options={emptyModelOptions} disabled onChange={() => undefined} ariaLabel={t("Default model", "默认模型")} />}
            <small>{models.length > 0 ? t("Used when no model is selected manually", "未手动切换时使用") : t("Add models on the Models tab first", "先在模型页签添加模型")}</small>
          </div>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Credentials", "凭据")}
        description={t(
          "API keys for this provider, with optional load balancing across multiple keys.",
          "当前供应商的 API 密钥，多密钥时可启用负载均衡。"
        )}
      >
        <div className="settings-field full">
          <ProviderApiKeysField
            // 切换供应商时重建：密钥框内部持有明文状态，
            // 复用实例会把上一个供应商的密钥露出来
            key={provider.id}
            providerId={provider.id}
            keys={providerKeys}
            selected={selectedProviderKey}
            balance={provider.api_key_balance === true}
            secretSentinel={secretSentinel}
            onRevealKey={onRevealKey}
            onChange={onKeysChange}
          />
          <small>{t("Use one selected key by default, or enable load balancing when multiple keys are configured. Environment variables can be referenced with `$env:VARIABLE_NAME`.", "默认使用一个选中的密钥；配置多个密钥后可以启用负载均衡。支持使用 `$env:VARIABLE_NAME` 引用环境变量。")}</small>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Connectivity", "连通性")}
        description={t(
          "Verify the endpoint and key with a real request.",
          "用一次真实请求验证地址与密钥。"
        )}
      >
        <div className="settings-field full">
          <ProviderConnectionTest
            key={`${provider.id}:${provider.default_model ?? ""}:${selectedProviderKey ?? ""}`}
            provider={provider}
            model={provider.default_model || undefined}
            selectedKeyId={selectedProviderKey}
          />
          <small>{t("Run a normal model response test or a separate tool-calling test with the selected key.", "可以使用当前选中的密钥分别测试普通模型响应和工具调用。")}</small>
        </div>
      </SettingsGroup>
    </>
  );
}

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
