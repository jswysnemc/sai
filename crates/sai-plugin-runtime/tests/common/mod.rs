#![allow(dead_code)]

use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{Capabilities, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct RecordingHost {
    pub requests: Mutex<Vec<HttpRequest>>,
}

#[async_trait]
impl PluginHost for RecordingHost {
    /// 【插件测试】【HTTP】记录真实宿主调用并返回固定数据。
    /// @param request 已授权请求；allowed_origins 为重定向来源约束
    /// @returns 可供 Lua 解析的 JSON 响应
    async fn http(
        &self,
        request: HttpRequest,
        allowed_origins: Vec<String>,
    ) -> Result<HttpResponse> {
        assert!(!allowed_origins.is_empty());
        self.requests.lock().unwrap().push(request);
        Ok(HttpResponse {
            status: 200,
            headers: BTreeMap::new(),
            text: r#"{"answer":42}"#.to_string(),
        })
    }
}

/// 【插件测试】【清单】创建有效的基础清单，无参数。
/// @returns 不需要宿主能力的插件清单
pub fn manifest() -> PluginManifest {
    PluginManifest::parse(
        &json!({
            "api_version": 1, "id": "test-plugin", "version": "1.0.0",
            "name": "测试插件", "description": "运行时契约测试", "entry": "init.lua"
        })
        .to_string(),
    )
    .unwrap()
}

/// 【插件测试】【源码】从单个脚本创建自包含插件。
/// @param source Lua 入口源码
/// @returns 可加载的源码快照
pub fn package(source: &str) -> PluginPackage {
    PluginPackage::new(
        manifest(),
        BTreeMap::from([("init.lua".to_string(), source.to_string())]),
    )
    .unwrap()
}

/// 【插件测试】【运行时】加载无需网络能力的测试脚本。
/// @param source Lua 入口源码
/// @returns 可调用的插件运行实例
pub fn runtime(source: &str) -> PluginRuntime {
    PluginRuntime::load(
        package(source),
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap()
}
