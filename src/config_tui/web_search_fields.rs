use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;

use super::form::{parse_bool_field, Field};

/// 构造 Web 搜索及各供应商的详细配置字段。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - Web 搜索配置表单字段
pub(super) fn web_search_fields(config: &AppConfig) -> Vec<Field> {
    vec![
        Field::boolean(t("Enabled", "启用"), config.plugins.web.enabled),
        Field::new(
            t("Default provider", "默认供应商"),
            config.plugins.web.default_provider.clone(),
        )
        .choices(&[
            "auto",
            "tinyfish",
            "tavily",
            "firecrawl",
            "anysearch",
            "searxng",
            "duckduckgo",
        ]),
        Field::new(
            t("Maximum results", "最大结果数量"),
            config.plugins.web.max_results.to_string(),
        ),
        Field::new(
            t("Timeout seconds", "超时秒数"),
            config.plugins.web.timeout_seconds.to_string(),
        ),
        Field::boolean(
            t("TinyFish enabled", "TinyFish 启用"),
            config.plugins.web.tinyfish_enabled,
        ),
        Field::textarea(
            t("TinyFish API Keys", "TinyFish 密钥"),
            config.plugins.web.tinyfish_api_keys.join("\n"),
        )
        .secret(),
        Field::new(
            t("TinyFish base URL", "TinyFish 服务地址"),
            config.plugins.web.tinyfish_base_url.clone(),
        ),
        Field::new(
            t("TinyFish default location", "TinyFish 默认位置"),
            config.plugins.web.tinyfish_default_location.clone(),
        ),
        Field::new(
            t("TinyFish default language", "TinyFish 默认语言"),
            config.plugins.web.tinyfish_default_language.clone(),
        ),
        Field::boolean(
            t("Tavily enabled", "Tavily 启用"),
            config.plugins.web.tavily_enabled,
        ),
        Field::textarea(
            t("Tavily API Keys", "Tavily 密钥"),
            config.plugins.web.tavily_api_keys.join("\n"),
        )
        .secret(),
        Field::new(
            t("Tavily base URL", "Tavily 服务地址"),
            config.plugins.web.tavily_base_url.clone(),
        ),
        Field::new(
            t("Tavily search depth", "Tavily 搜索深度"),
            config.plugins.web.tavily_search_depth.clone(),
        )
        .choices(&["basic", "advanced"]),
        Field::boolean(
            t("Tavily include answer", "Tavily 附带生成答案"),
            config.plugins.web.tavily_include_answer,
        ),
        Field::boolean(
            t("Tavily include raw content", "Tavily 附带原始正文"),
            config.plugins.web.tavily_include_raw_content,
        ),
        Field::boolean(
            t("Firecrawl enabled", "Firecrawl 启用"),
            config.plugins.web.firecrawl_enabled,
        ),
        Field::textarea(
            t("Firecrawl API Keys", "Firecrawl 密钥"),
            config.plugins.web.firecrawl_api_keys.join("\n"),
        )
        .secret(),
        Field::new(
            t("Firecrawl base URL", "Firecrawl 服务地址"),
            config.plugins.web.firecrawl_base_url.clone(),
        ),
        Field::boolean(
            t("Firecrawl only main content", "Firecrawl 仅保留主要正文"),
            config.plugins.web.firecrawl_only_main_content,
        ),
        Field::boolean(
            t("AnySearch enabled", "AnySearch 启用"),
            config.plugins.web.anysearch_enabled,
        ),
        Field::textarea(
            t("AnySearch API Keys", "AnySearch 密钥"),
            config.plugins.web.anysearch_api_keys.join("\n"),
        )
        .secret(),
        Field::new(
            t("AnySearch base URL", "AnySearch 服务地址"),
            config.plugins.web.anysearch_base_url.clone(),
        ),
        Field::boolean(
            t("SearXNG enabled", "SearXNG 启用"),
            config.plugins.web.searxng_enabled,
        ),
        Field::new(
            t("SearXNG URL", "SearXNG 地址"),
            config.plugins.web.searxng_base_url.clone(),
        ),
        Field::new(
            t("SearXNG language", "SearXNG 语言"),
            config.plugins.web.searxng_language.clone(),
        ),
        Field::new(
            t("SearXNG safe search", "SearXNG 安全搜索"),
            config.plugins.web.searxng_safe_search.to_string(),
        )
        .choices(&["0", "1", "2"]),
        Field::boolean(
            t("DuckDuckGo enabled", "DuckDuckGo 启用"),
            config.plugins.web.duckduckgo_enabled,
        ),
    ]
}

/// 将 Web 搜索表单字段完整校验后写回配置。
///
/// 参数:
/// - `config`: 待更新应用配置
/// - `fields`: Web 搜索表单字段
///
/// 返回:
/// - 全部字段合法时写入配置，否则返回解析或校验错误
pub(super) fn apply_web_search_fields(config: &mut AppConfig, fields: &[Field]) -> Result<()> {
    let mut next = config.plugins.web.clone();
    // 1. 先在副本中解析全部字段，避免解析失败时只写入部分配置
    next.enabled = parse_bool_field(&fields[0].value)?;
    next.default_provider = fields[1].value.trim().to_string();
    next.max_results = fields[2].value.trim().parse()?;
    next.timeout_seconds = fields[3].value.trim().parse()?;
    next.tinyfish_enabled = parse_bool_field(&fields[4].value)?;
    next.tinyfish_api_keys = parse_key_list(&fields[5].value);
    next.tinyfish_base_url = normalize_url(&fields[6].value);
    next.tinyfish_default_location = fields[7].value.trim().to_string();
    next.tinyfish_default_language = fields[8].value.trim().to_string();
    next.tavily_enabled = parse_bool_field(&fields[9].value)?;
    next.tavily_api_keys = parse_key_list(&fields[10].value);
    next.tavily_base_url = normalize_url(&fields[11].value);
    next.tavily_search_depth = fields[12].value.trim().to_string();
    next.tavily_include_answer = parse_bool_field(&fields[13].value)?;
    next.tavily_include_raw_content = parse_bool_field(&fields[14].value)?;
    next.firecrawl_enabled = parse_bool_field(&fields[15].value)?;
    next.firecrawl_api_keys = parse_key_list(&fields[16].value);
    next.firecrawl_base_url = normalize_url(&fields[17].value);
    next.firecrawl_only_main_content = parse_bool_field(&fields[18].value)?;
    next.anysearch_enabled = parse_bool_field(&fields[19].value)?;
    next.anysearch_api_keys = parse_key_list(&fields[20].value);
    next.anysearch_base_url = normalize_url(&fields[21].value);
    next.searxng_enabled = parse_bool_field(&fields[22].value)?;
    next.searxng_base_url = normalize_url(&fields[23].value);
    next.searxng_language = fields[24].value.trim().to_string();
    next.searxng_safe_search = fields[25].value.trim().parse()?;
    next.duckduckgo_enabled = parse_bool_field(&fields[26].value)?;
    // 2. 完整校验通过后再替换当前配置
    next.validate()?;
    config.plugins.web = next;
    Ok(())
}

/// 解析多行或逗号分隔的接口密钥。
///
/// 参数:
/// - `value`: 表单输入
///
/// 返回:
/// - 去除空白后的密钥列表
fn parse_key_list(value: &str) -> Vec<String> {
    value
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

/// 统一移除服务地址末尾斜杠。
///
/// 参数:
/// - `value`: 表单输入地址
///
/// 返回:
/// - 去除首尾空白和末尾斜杠的地址
fn normalize_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}
