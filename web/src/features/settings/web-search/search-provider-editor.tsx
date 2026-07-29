import { Select } from "../../../shared/ui/select/select";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";
import {
  normalizeWebSearchSelection,
  webSearchProviderAvailable,
  webSearchProviderEnabled,
  webSearchProviderStatus,
  type WebSearchConfig,
  type WebSearchProviderId
} from "./web-search-config";
import {
  getSearchProvider,
  searchProviderDescription,
  searchProviderEnvironmentVariable
} from "./search-provider-catalog";
import { SearchApiKeysField } from "./search-api-keys-field";

type SearchProviderEditorProps = {
  providerId: WebSearchProviderId;
  config: WebSearchConfig;
  secretSentinel: string;
  onChange: (config: WebSearchConfig) => void;
};

type ProviderSettingsProps = {
  config: WebSearchConfig;
  secretSentinel: string;
  update: (patch: Partial<WebSearchConfig>) => void;
};

/**
 * 渲染单个搜索供应商的连接和检索参数。
 *
 * @param props 供应商标识、搜索配置、敏感字段占位符和更新回调
 * @returns 供应商详细设置编辑区
 */
export function SearchProviderEditor({
  providerId,
  config,
  secretSentinel,
  onChange
}: SearchProviderEditorProps) {
  const { locale, t } = useI18n();
  const provider = getSearchProvider(providerId);
  const enabled = webSearchProviderEnabled(config, providerId);
  // 开关反映用户意图，而实际可路由状态还要求具备必要连接信息
  const available = webSearchProviderAvailable(config, providerId);
  const status = webSearchProviderStatus(config, providerId);

  /**
   * 合并供应商局部配置，并同步校正默认供应商。
   *
   * @param patch 待写入的供应商字段
   * @returns 无返回值
   */
  const update = (patch: Partial<WebSearchConfig>): void => {
    onChange(normalizeWebSearchSelection({ ...config, ...patch }));
  };

  return (
    <section className={available ? "settings-editor search-provider-editor" : "settings-editor search-provider-editor is-disabled"}>
      <EditorHeader
        kicker={t("Search provider", "搜索供应商")}
        title={provider.label}
        description={searchProviderDescription(provider, locale)}
        actions={(
          <div className="search-provider-header-actions">
            <span className={"search-provider-status " + status}>
              {providerStatusText(status, t)}
            </span>
            <label className="settings-switch">
              <input
                type="checkbox"
                checked={enabled}
                aria-label={t("Enable provider", "启用供应商")}
                onChange={(event) => update({
                  [providerId + "_enabled"]: event.target.checked
                } as Partial<WebSearchConfig>)}
              />
              <span aria-hidden="true" />
              <strong>{enabled ? t("Enabled", "已启用") : t("Disabled", "已停用")}</strong>
            </label>
          </div>
        )}
      />
      <div className="search-provider-settings">
        <ProviderSettings
          providerId={providerId}
          config={config}
          secretSentinel={secretSentinel}
          update={update}
        />
      </div>
    </section>
  );
}

/**
 * 按供应商标识挂载对应配置字段。
 *
 * @param props 供应商标识和通用供应商设置参数
 * @returns 对应供应商设置
 */
function ProviderSettings({
  providerId,
  ...props
}: ProviderSettingsProps & { providerId: WebSearchProviderId }) {
  switch (providerId) {
    case "tinyfish":
      return <TinyFishSettings {...props} />;
    case "tavily":
      return <TavilySettings {...props} />;
    case "firecrawl":
      return <FirecrawlSettings {...props} />;
    case "anysearch":
      return <AnySearchSettings {...props} />;
    case "searxng":
      return <SearxngSettings {...props} />;
    case "duckduckgo":
      return <DuckDuckGoSettings />;
  }
}

/**
 * 渲染 TinyFish 供应商设置。
 *
 * @param props 搜索配置、占位符和更新方法
 * @returns TinyFish 设置
 */
function TinyFishSettings({ config, secretSentinel, update }: ProviderSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <SettingsGroup title={t("Connection", "连接")} description={t("Credentials and API endpoint.", "配置凭据与接口地址。")}>
        <div className="settings-form-grid">
          <SearchApiKeysField
            keys={config.tinyfish_api_keys}
            environmentVariable={searchProviderEnvironmentVariable("tinyfish")}
            secretSentinel={secretSentinel}
            onChange={(keys) => update({ tinyfish_api_keys: keys })}
          />
          <TextField
            label={t("API endpoint", "接口地址")}
            hint="tinyfish_base_url"
            value={config.tinyfish_base_url}
            onChange={(value) => update({ tinyfish_base_url: value })}
          />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Request preferences", "请求偏好")} description={t("Optional defaults sent with each search.", "每次搜索附带的可选默认参数。")}>
        <div className="settings-form-grid">
          <TextField
            label={t("Default location", "默认位置")}
            hint={t("Leave empty to omit location.", "留空则不指定位置。")}
            value={config.tinyfish_default_location}
            onChange={(value) => update({ tinyfish_default_location: value })}
          />
          <TextField
            label={t("Default language", "默认语言")}
            hint={t("Leave empty to use the service default.", "留空则使用服务默认语言。")}
            value={config.tinyfish_default_language}
            onChange={(value) => update({ tinyfish_default_language: value })}
          />
        </div>
      </SettingsGroup>
    </>
  );
}

/**
 * 渲染 Tavily 供应商设置。
 *
 * @param props 搜索配置、占位符和更新方法
 * @returns Tavily 设置
 */
function TavilySettings({ config, secretSentinel, update }: ProviderSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <SettingsGroup title={t("Connection", "连接")} description={t("Credentials and API endpoint.", "配置凭据与接口地址。")}>
        <div className="settings-form-grid">
          <SearchApiKeysField
            keys={config.tavily_api_keys}
            environmentVariable={searchProviderEnvironmentVariable("tavily")}
            secretSentinel={secretSentinel}
            onChange={(keys) => update({ tavily_api_keys: keys })}
          />
          <TextField
            label={t("API endpoint", "接口地址")}
            hint="tavily_base_url"
            value={config.tavily_base_url}
            onChange={(value) => update({ tavily_base_url: value })}
          />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Result detail", "结果详情")} description={t("Balance response detail against latency and payload size.", "平衡结果详情、耗时与响应大小。")}>
        <div className="settings-form-grid">
          <div className="settings-field">
            <span>{t("Search depth", "搜索深度")}</span>
            <Select<"basic" | "advanced">
              value={config.tavily_search_depth}
              options={[
                { value: "basic", label: t("Basic", "基础"), description: t("Lower latency", "耗时较低") },
                { value: "advanced", label: t("Advanced", "深入"), description: t("Broader retrieval", "检索范围更广") }
              ]}
              ariaLabel={t("Tavily search depth", "Tavily 搜索深度")}
              onChange={(depth) => update({ tavily_search_depth: depth })}
            />
            <small>tavily_search_depth</small>
          </div>
          <BooleanField
            label={t("Include generated answer", "附带生成答案")}
            hint={t("Requests Tavily's synthesized answer.", "请求 Tavily 返回综合答案。")}
            checked={config.tavily_include_answer}
            onChange={(checked) => update({ tavily_include_answer: checked })}
          />
          <BooleanField
            label={t("Include raw content", "附带原始正文")}
            hint={t("Includes extracted page content in results.", "在结果中附带提取后的页面正文。")}
            checked={config.tavily_include_raw_content}
            onChange={(checked) => update({ tavily_include_raw_content: checked })}
          />
        </div>
      </SettingsGroup>
    </>
  );
}

/**
 * 渲染 Firecrawl 供应商设置。
 *
 * @param props 搜索配置、占位符和更新方法
 * @returns Firecrawl 设置
 */
function FirecrawlSettings({ config, secretSentinel, update }: ProviderSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <SettingsGroup title={t("Connection", "连接")} description={t("Credentials and API endpoint.", "配置凭据与接口地址。")}>
        <div className="settings-form-grid">
          <SearchApiKeysField
            keys={config.firecrawl_api_keys}
            environmentVariable={searchProviderEnvironmentVariable("firecrawl")}
            secretSentinel={secretSentinel}
            onChange={(keys) => update({ firecrawl_api_keys: keys })}
          />
          <TextField
            label={t("API endpoint", "接口地址")}
            hint="firecrawl_base_url"
            value={config.firecrawl_base_url}
            onChange={(value) => update({ firecrawl_base_url: value })}
          />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Content extraction", "正文提取")}>
        <div className="settings-form-grid">
          <BooleanField
            label={t("Only main content", "仅保留主要正文")}
            hint={t("Drops navigation, footer, and other page chrome.", "移除导航、页脚等非正文内容。")}
            checked={config.firecrawl_only_main_content}
            onChange={(checked) => update({ firecrawl_only_main_content: checked })}
          />
        </div>
      </SettingsGroup>
    </>
  );
}

/**
 * 渲染 AnySearch 供应商设置。
 *
 * @param props 搜索配置、占位符和更新方法
 * @returns AnySearch 设置
 */
function AnySearchSettings({ config, secretSentinel, update }: ProviderSettingsProps) {
  const { t } = useI18n();
  return (
    <SettingsGroup title={t("Connection", "连接")} description={t("Credentials and API endpoint.", "配置凭据与接口地址。")}>
      <div className="settings-form-grid">
        <SearchApiKeysField
          keys={config.anysearch_api_keys}
          environmentVariable={searchProviderEnvironmentVariable("anysearch")}
          secretSentinel={secretSentinel}
          onChange={(keys) => update({ anysearch_api_keys: keys })}
        />
        <TextField
          label={t("API endpoint", "接口地址")}
          hint="anysearch_base_url"
          value={config.anysearch_base_url}
          onChange={(value) => update({ anysearch_base_url: value })}
        />
      </div>
    </SettingsGroup>
  );
}

/**
 * 渲染 SearXNG 供应商设置。
 *
 * @param props 搜索配置和更新方法
 * @returns SearXNG 设置
 */
function SearxngSettings({ config, update }: ProviderSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <SettingsGroup title={t("Connection", "连接")} description={t("Point to a SearXNG instance that exposes JSON search.", "填写支持 JSON 搜索的 SearXNG 实例地址。")}>
        <div className="settings-form-grid">
          <TextField
            label={t("Instance URL", "实例地址")}
            hint={t("Required before SearXNG can be selected.", "填写后才可使用 SearXNG。")}
            value={config.searxng_base_url}
            onChange={(value) => update({ searxng_base_url: value })}
          />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Search preferences", "搜索偏好")}>
        <div className="settings-form-grid">
          <TextField
            label={t("Language", "语言")}
            hint={t("Use auto or a SearXNG language code.", "填写 auto 或 SearXNG 支持的语言代码。")}
            value={config.searxng_language}
            onChange={(value) => update({ searxng_language: value })}
          />
          <div className="settings-field">
            <span>{t("Safe search", "安全搜索")}</span>
            <Select<"0" | "1" | "2">
              value={String(config.searxng_safe_search) as "0" | "1" | "2"}
              options={[
                { value: "0", label: t("Off", "关闭") },
                { value: "1", label: t("Moderate", "适中") },
                { value: "2", label: t("Strict", "严格") }
              ]}
              ariaLabel={t("SearXNG safe search", "SearXNG 安全搜索")}
              onChange={(value) => update({ searxng_safe_search: Number(value) })}
            />
            <small>searxng_safe_search</small>
          </div>
        </div>
      </SettingsGroup>
    </>
  );
}

/**
 * 渲染 DuckDuckGo 内置供应商说明。
 *
 * @returns DuckDuckGo 说明
 */
function DuckDuckGoSettings() {
  const { t } = useI18n();
  return (
    <SettingsGroup
      title={t("Built-in fallback", "内置回退")}
      description={t(
        "This provider runs through the bundled search integration and does not require credentials or an endpoint.",
        "该供应商通过内置搜索集成运行，不需要配置凭据或服务地址。"
      )}
    >
      <div className="search-provider-note">
        <strong>{t("No additional configuration", "无需其他配置")}</strong>
        <p>{t(
          "Keep it enabled when automatic routing should retain a credential-free fallback.",
          "如需为自动路由保留无需凭据的回退方案，请保持启用。"
        )}</p>
      </div>
    </SettingsGroup>
  );
}

/**
 * 渲染单行文本配置字段。
 *
 * @param props 字段名称、说明、值和更新回调
 * @returns 文本输入字段
 */
function TextField({
  label,
  hint,
  value,
  onChange
}: {
  label: string;
  hint: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="settings-field">
      <span>{label}</span>
      <input
        type="text"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        autoComplete="off"
      />
      <small>{hint}</small>
    </label>
  );
}

/**
 * 渲染布尔配置开关。
 *
 * @param props 字段名称、说明、状态和更新回调
 * @returns 布尔开关字段
 */
function BooleanField({
  label,
  hint,
  checked,
  onChange
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="settings-toggle-field">
      <span>
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}

/**
 * 返回供应商配置状态文案。
 *
 * @param status 供应商配置状态
 * @param t 双语文本选择方法
 * @returns 本地化状态文案
 */
function providerStatusText(
  status: "configured" | "builtin" | "missing",
  t: (en: string, zh: string) => string
): string {
  if (status === "configured") return t("Configured", "已配置");
  if (status === "builtin") return t("Built in", "内置");
  return t("Not configured here", "未在此配置");
}
