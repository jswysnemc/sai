use crate::plugins::config::PluginSetting;
use crate::plugins::discovery::{PluginDescriptor, PluginSource};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{Capabilities, PluginManifest, PluginPackage};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct FixtureHost {
    pub requests: Mutex<Vec<HttpRequest>>,
    pub responses: Mutex<VecDeque<HttpResponse>>,
}

impl FixtureHost {
    /// 【插件测试】【HTTP 样本】按请求顺序提供响应，避免依赖外网和系统时间。
    /// @param responses 状态码和正文列表
    /// @returns 可注入实际 Lua 运行时的宿主
    pub fn new(responses: &[(u16, &str)]) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(
                responses
                    .iter()
                    .map(|(status, text)| HttpResponse {
                        status: *status,
                        headers: BTreeMap::new(),
                        text: text.to_string(),
                    })
                    .collect(),
            ),
        }
    }
}

#[async_trait]
impl PluginHost for FixtureHost {
    /// 【插件测试】【请求捕获】记录真实插件请求，并返回下一个固定响应。
    /// @param request 实际请求；allowed_origins 为运行时交付的授权集合
    /// @returns 下一个响应，没有对应样本时失败
    async fn http(
        &self,
        request: HttpRequest,
        allowed_origins: Vec<String>,
    ) -> Result<HttpResponse> {
        Capabilities {
            http: allowed_origins.into_iter().collect(),
        }
        .authorize_url(&request.url)?;
        self.requests.lock().unwrap().push(request);
        match self.responses.lock().unwrap().pop_front() {
            Some(response) => Ok(response),
            None => bail!("no HTTP fixture for request"),
        }
    }
}

/// 【插件测试】【外部描述】从独立源码创建不带隐式网络授权的外部插件。
/// @param id 插件 ID；source 为 Lua 源码
/// @returns 可直接送入真实注册适配的描述
pub(super) fn descriptor(id: &str, source: &str) -> PluginDescriptor {
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1, "id":id, "version":"1.0.0", "name":id,
            "description":"Test plugin", "entry":"init.lua",
            "capabilities":{"http":["https://example.test"]},
        })
        .to_string(),
    )
    .unwrap();
    PluginDescriptor {
        package: PluginPackage::new(
            manifest,
            BTreeMap::from([("init.lua".into(), source.into())]),
        )
        .unwrap(),
        source: PluginSource::Installed(std::path::PathBuf::from(id)),
        setting: PluginSetting {
            enabled: true,
            ..Default::default()
        },
    }
}

/// 【插件测试】【目录样本】把固定描述写入独立目录，供安装和发现流程使用。
/// @param root 目标目录；descriptor 为测试包
/// @returns 无
pub(super) fn write_package(root: &std::path::Path, descriptor: &PluginDescriptor) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("sai-plugin.json"),
        serde_json::to_vec(&descriptor.package.manifest).unwrap(),
    )
    .unwrap();
    for (path, source) in descriptor.package.sources() {
        std::fs::write(root.join(path), source).unwrap();
    }
}
