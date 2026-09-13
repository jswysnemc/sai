use crate::plugins::config::PluginSetting;
use crate::plugins::discovery::{PluginDescriptor, PluginSource};
use sai_plugin_runtime::host::PluginHost;
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) const TOOL: &str = "web_search";
pub(super) const RESULT: &str =
    r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org","content":"语言文档"}]}"#;
pub(super) const HTML: &str = r#"<a class="result__a" href="https://example.test/?a=1&amp;b=2">输入法&nbsp; Rust</a><a class="result__snippet">中文　与 Lua</a>"#;

/// 【搜索测试】【独立样本】以普通外部身份加载示例，显式提供冻结业务对照所需设置和来源
/// @param settings 当前样本的覆盖设置
/// @returns 不使用主配置或兼容投影的源码描述；实际安装另由生命周期测试覆盖
pub(super) fn descriptor(settings: Value) -> PluginDescriptor {
    let mut package = super::example_support::package("web-search");
    let mut merged = json!({
        "tinyfish_api_keys":["tiny-key"], "tavily_api_keys":["tavily-key"],
        "firecrawl_api_keys":["firecrawl-key"], "anysearch_api_keys":["anysearch-key"],
        "searxng_base_url":"https://search.example.test",
    });
    merged
        .as_object_mut()
        .unwrap()
        .extend(settings.as_object().unwrap().clone());
    // 1. 【搜索测试】【样本授权】测试来源写入样本清单，产品配置不会执行这一派生
    for field in [
        "tinyfish_base_url",
        "tavily_base_url",
        "firecrawl_base_url",
        "anysearch_base_url",
        "searxng_base_url",
    ] {
        if let Some(endpoint) = merged[field].as_str() {
            if let Ok(mut url) = reqwest::Url::parse(endpoint) {
                if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
                    package
                        .manifest
                        .capabilities
                        .http
                        .insert(url.origin().ascii_serialization());
                    if matches!(
                        field,
                        "tavily_base_url" | "firecrawl_base_url" | "anysearch_base_url"
                    ) {
                        url.set_query(None);
                        url.set_fragment(None);
                        package
                            .manifest
                            .capabilities
                            .http_read_only_post
                            .insert(url.to_string());
                    }
                }
            }
        }
    }
    let grants = package.manifest.capabilities.clone();
    PluginDescriptor {
        package,
        source: PluginSource::Installed(super::example_support::directory("web-search")),
        setting: PluginSetting {
            enabled: true,
            settings: merged,
            grants: Some(grants),
        },
    }
}

/// 【搜索测试】【运行时】从示例源码与显式测试权限加载 Lua
/// @param settings 插件设置；host 为测试网络实现
/// @returns 独立 Lua 实例
pub(super) fn runtime(settings: Value, host: Arc<dyn PluginHost>) -> PluginRuntime {
    let descriptor = descriptor(settings);
    PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host,
    )
    .unwrap()
}

/// 【搜索测试】【查询入口】通过工具接口执行指定供应商查询
/// @param plugin 插件实例；provider 为供应商名称
/// @returns 查询 Rust 所产生的完整 Markdown
pub(super) async fn search(plugin: &PluginRuntime, provider: &str) -> String {
    plugin
        .call_tool(
            TOOL,
            json!({"query":"Rust","provider":provider}),
            InvocationContext::default(),
        )
        .await
        .unwrap()
}
