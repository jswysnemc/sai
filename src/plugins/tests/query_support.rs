use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{Capabilities, PluginPackage, PluginRuntime};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

pub(super) const QUERIES: [(&str, &str); 3] = [
    ("weather", "get_weather"),
    ("exchange-rate", "get_exchange_rate"),
    ("moegirl", "query_moegirl"),
];

/// 【查询插件测试】【响应队列】同时表达 HTTP 状态、正文与传输失败，不访问外部服务。
pub(super) struct QueryHost {
    pub requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<VecDeque<std::result::Result<HttpResponse, String>>>,
}

impl QueryHost {
    /// 【查询插件测试】【宿主样本】按顺序提供正文或传输错误，并保存每次真实请求。
    /// @param responses HTTP 状态与正文，或不包含凭据的固定传输错误
    /// @returns 可注入正式运行时的宿主
    pub fn new(responses: &[std::result::Result<(u16, &str), &str>]) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(
                responses
                    .iter()
                    .map(|response| match response {
                        Ok((status, text)) => Ok(HttpResponse {
                            status: *status,
                            headers: BTreeMap::new(),
                            text: text.to_string(),
                        }),
                        Err(error) => Err(error.to_string()),
                    })
                    .collect(),
            ),
        }
    }
}

#[async_trait]
impl PluginHost for QueryHost {
    /// 【查询插件测试】【网络契约】执行授权检查和正文大小限制，再交付固定响应。
    /// @param request 插件请求；capabilities 为有效授权；allow_writes 为可信写入权限
    /// @returns 下一条响应或传输错误
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        capabilities.authorize_request(&request.method, &request.url, allow_writes)?;
        let maximum = request.max_bytes;
        self.requests.lock().unwrap().push(request);
        match self.responses.lock().unwrap().pop_front() {
            Some(Ok(response)) if response.text.len() <= maximum => Ok(response),
            Some(Ok(_)) => bail!("plugin HTTP response exceeds byte limit"),
            Some(Err(error)) => bail!(error),
            None => bail!("query HTTP fixture exhausted"),
        }
    }
}

/// 【查询插件测试】【真实源码】从发布目录取得指定内置包。
/// @param id 包标识
/// @returns 真实清单与源码快照
pub(super) fn package(id: &str) -> PluginPackage {
    crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == id)
        .unwrap()
}

/// 【查询插件测试】【真实运行时】使用包的完整声明和显式测试设置加载 Lua。
/// @param id 包标识；settings 为设置；host 为可观察宿主
/// @returns 独立运行时实例
pub(super) fn runtime(id: &str, settings: Value, host: Arc<dyn PluginHost>) -> PluginRuntime {
    let package = package(id);
    let grants = package.manifest.capabilities.clone();
    PluginRuntime::load(package, settings, grants, host).unwrap()
}

/// 【查询插件测试】【真实发现】从应用配置和独立路径读取固定源码、设置及授权快照。
/// @param config 主配置；paths 为测试路径；id 为待查内置包
/// @returns 与正式注册入口一致的插件描述
pub(super) fn discover(
    config: &crate::config::AppConfig,
    paths: &crate::paths::SaiPaths,
    id: &str,
) -> crate::plugins::discovery::PluginDescriptor {
    let found = crate::plugins::discover(config, paths);
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    found
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == id)
        .unwrap()
}
