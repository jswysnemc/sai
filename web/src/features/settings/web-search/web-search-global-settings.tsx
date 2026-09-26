import { Select } from "../../../shared/ui/select/select";
import { SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";
import {
  WEB_SEARCH_PROVIDER_IDS,
  webSearchProviderAvailable,
  type WebSearchConfig,
  type WebSearchDefaultProvider
} from "./web-search-config";
import { getSearchProvider } from "./search-provider-catalog";

type WebSearchGlobalSettingsProps = {
  config: WebSearchConfig;
  onChange: (config: WebSearchConfig) => void;
};

/**
 * 渲染 Web 搜索的默认路由、结果数量和超时设置。
 *
 * @param props Web 搜索配置和更新回调
 * @returns 全局搜索行为表单
 */
export function WebSearchGlobalSettings({
  config,
  onChange
}: WebSearchGlobalSettingsProps) {
  const { t } = useI18n();
  const providerOptions = [
    {
      value: "auto" as const,
      label: t("Automatic", "自动选择"),
      description: t("Try enabled providers in the builtin priority order", "按内置优先级尝试已启用的供应商")
    },
    ...WEB_SEARCH_PROVIDER_IDS
      .filter((provider) => webSearchProviderAvailable(config, provider))
      .map((provider) => ({
        value: provider,
        label: getSearchProvider(provider).label
      }))
  ];

  return (
    <SettingsGroup
      title={t("Search behavior", "搜索行为")}
      description={t(
        "Shared defaults used when a tool call does not specify provider, result count, or timeout.",
        "工具调用未指定供应商、结果数量或超时时使用这些默认值。"
      )}
    >
      <div className="settings-form-grid web-search-global-grid">
        <div className="settings-field">
          <span>{t("Default provider", "默认供应商")}</span>
          <Select<WebSearchDefaultProvider>
            value={config.default_provider}
            options={providerOptions}
            ariaLabel={t("Default search provider", "默认搜索供应商")}
            menuMinimumWidth={250}
            onChange={(defaultProvider) => onChange({
              ...config,
              default_provider: defaultProvider
            })}
          />
          <small>{t(
            "Disabled providers and SearXNG without an endpoint are omitted.",
            "此列表不显示已停用的供应商，也不显示尚未配置地址的 SearXNG。"
          )}</small>
        </div>
        <label className="settings-field">
          <span>{t("Maximum results", "最大结果数量")}</span>
          <input
            type="number"
            min={1}
            max={10}
            value={config.max_results}
            onChange={(event) => onChange({
              ...config,
              max_results: parseEditingNumber(event.target.value, config.max_results)
            })}
            onBlur={(event) => onChange({
              ...config,
              max_results: clampNumber(event.target.value, 1, 10, config.max_results)
            })}
          />
          <small>{t("Allowed range: 1 to 10 results.", "允许范围：1 至 10 条结果。")}</small>
        </label>
        <label className="settings-field">
          <span>{t("Request timeout", "请求超时")}</span>
          <input
            type="number"
            min={1}
            max={120}
            value={config.timeout_seconds}
            onChange={(event) => onChange({
              ...config,
              timeout_seconds: parseEditingNumber(event.target.value, config.timeout_seconds)
            })}
            onBlur={(event) => onChange({
              ...config,
              timeout_seconds: clampNumber(event.target.value, 1, 120, config.timeout_seconds)
            })}
          />
          <small>{t("Seconds, from 1 to 120.", "单位为秒，允许范围为 1 至 120。")}</small>
        </label>
      </div>
    </SettingsGroup>
  );
}

/**
 * 解析编辑过程中的数值输入，不做范围收敛。
 *
 * 逐字符钳制会让用户无法输入两位以上的数值（例如输入 45 会先被截断），
 * 因此编辑期间只过滤非法字符，范围收敛留到失焦时执行。
 *
 * @param value 输入框字符串
 * @param fallback 无法解析时使用的值
 * @returns 输入对应的整数
 */
function parseEditingNumber(value: string, fallback: number): number {
  if (value.trim() === "") return 0;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.round(parsed);
}

/**
 * 将数值输入约束到指定范围。
 *
 * @param value 输入框字符串
 * @param minimum 最小值
 * @param maximum 最大值
 * @param fallback 无法解析时使用的值
 * @returns 合法范围内的整数
 */
function clampNumber(
  value: string,
  minimum: number,
  maximum: number,
  fallback: number
): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.round(parsed)));
}
