use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::{save_config, PluginConfig, PluginSetting};
use crate::plugins::discovery::PluginDescriptor;
use sai_plugin_runtime::host::PluginHost;
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) const TOOL: &str = "web_search";
pub(super) const RESULT: &str =
    r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org","content":"语言文档"}]}"#;
pub(super) const HTML: &str = r#"<a class="result__a" href="https://example.test/?a=1&amp;b=2">输入法&nbsp; Rust</a><a class="result__snippet">中文　与 Lua</a>"#;

/// 【搜索测试】【真实发现】通过独立插件配置生成与应用一致的搜索描述。
/// @param settings 待覆盖的插件设置
/// @returns 拥有固定源码、设置及授权的描述，测试密钥不会访问真实服务
pub(super) fn descriptor(settings: Value) -> PluginDescriptor {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.tinyfish_api_keys = vec!["tiny-key".into()];
    config.plugins.web.tavily_api_keys = vec!["tavily-key".into()];
    config.plugins.web.firecrawl_api_keys = vec!["firecrawl-key".into()];
    config.plugins.web.anysearch_api_keys = vec!["anysearch-key".into()];
    config.plugins.web.searxng_base_url = "https://search.example.test".into();
    let mut plugins = PluginConfig::default();
    plugins.plugins.insert(
        "web-search".into(),
        PluginSetting {
            enabled: true,
            settings,
            ..Default::default()
        },
    );
    save_config(&paths, &plugins).unwrap();
    let found = crate::plugins::discover(&config, &paths);
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    found
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == "web-search")
        .unwrap()
}

/// 【搜索测试】【运行时】加载真实搜索包并注入可观察的 HTTP 宿主。
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

/// 【搜索测试】【查询入口】通过工具接口执行指定供应商查询。
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
