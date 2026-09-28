import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, EmptyGuide, FieldGrid, SettingsField, SettingsPanel, SkTextInput, SwitchField } from "../kit";
import { SearchApiKeysField } from "./search-api-keys-field";
import { searchProviderEnvironmentVariable } from "./search-provider-catalog";
import type { WebSearchConfig, WebSearchProviderId } from "./web-search-config";
import { SEARCH_PROVIDER_FIELDS } from "./search-provider-fields-data";

/**
 * 【网页搜索】【供应商字段】按供应商元数据组合凭据、地址与检索选项。
 * @param props 供应商标识、搜索配置、脱敏标记与局部更新回调
 * @returns 当前供应商字段面板
 */
export function SearchProviderFields({ providerId, config, secretSentinel, update }: {
  providerId: WebSearchProviderId;
  config: WebSearchConfig;
  secretSentinel: string;
  update: (patch: Partial<WebSearchConfig>) => void;
}) {
  const { t } = useI18n();
  if (providerId === "duckduckgo") return <EmptyGuide title={t("Built-in fallback", "内置回退")} description={t("No credentials or endpoint required. Keep enabled to retain a credential-free fallback.", "无需凭据或接口地址，启用后可作为无需凭据的回退方案。")} />;
  const keysField = `${providerId}_api_keys`;
  const fields = SEARCH_PROVIDER_FIELDS[providerId];
  return <>
    <SettingsPanel title={t("Connection", "连接")}>
      <FieldGrid>
        {providerId !== "searxng" && <SearchApiKeysField anchor={`web-search.${keysField}`} keys={Array.isArray(config[keysField]) ? config[keysField] as string[] : []} environmentVariable={searchProviderEnvironmentVariable(providerId)} secretSentinel={secretSentinel} onChange={(keys) => update({ [keysField]: keys })} />}
        <SettingsField label={t("API endpoint", "接口地址")} configKey={`plugins.web.${providerId}_base_url`} anchor={`web-search.${providerId}_base_url`} span="full" hint={providerId === "searxng" ? t("A SearXNG instance that supports JSON search is required.", "需填写支持 JSON 搜索的 SearXNG 实例地址。"): undefined}>
          <SkTextInput mono value={String(config[`${providerId}_base_url`] ?? "")} onChange={(value) => update({ [`${providerId}_base_url`]: value })} />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
    {!!fields.length && <SettingsPanel title={t("Search preferences", "搜索偏好")}><FieldGrid>
      {fields.map((field) => {
        const props = { label: t(field.en, field.zh), hint: t(field.hintEn, field.hintZh), configKey: `plugins.web.${field.key}`, anchor: `web-search.${field.key}` };
        if (field.kind === "switch") return <SwitchField key={field.key} {...props} checked={config[field.key] === true} onChange={(value) => update({ [field.key]: value })} />;
        return <SettingsField key={field.key} {...props}>
          {field.options ? <ChoicePills value={String(config[field.key])} options={field.options.map(([value, en, zh]) => ({ value, label: t(en, zh) }))} onChange={(value) => update({ [field.key]: field.numeric ? Number(value) : value })} />
            : <SkTextInput value={String(config[field.key] ?? "")} onChange={(value) => update({ [field.key]: value })} />}
        </SettingsField>;
      })}
    </FieldGrid></SettingsPanel>}
  </>;
}
