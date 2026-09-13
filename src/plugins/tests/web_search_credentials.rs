use super::support::FixtureHost;
use super::web_search_support::{descriptor, RESULT, TOOL};
use anyhow::{ensure, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

struct CredentialHost {
    http: FixtureHost,
    environment: Mutex<BTreeMap<String, String>>,
    reads: Mutex<Vec<String>>,
}

impl CredentialHost {
    /// 【搜索凭据测试】【隔离宿主】提供固定网络与环境值，不读取进程真实环境
    /// @returns 可观察环境读取和 HTTP 请求的宿主
    fn new() -> Self {
        Self {
            http: FixtureHost::new(&[(200, RESULT), (200, RESULT)]),
            environment: Mutex::new(BTreeMap::from([(
                "TAVILY_API_KEY".into(),
                "first-key".into(),
            )])),
            reads: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl PluginHost for CredentialHost {
    /// 【搜索凭据测试】【环境读取】核对精确授权后返回当前样本值
    /// @param name 变量名；capabilities 为清单和用户授权的交集
    /// @returns 样本环境值或缺失
    fn environment(&self, name: &str, capabilities: &Capabilities) -> Result<Option<String>> {
        ensure!(
            capabilities.system.environment.contains(name),
            "environment is not allowed"
        );
        self.reads.lock().unwrap().push(name.into());
        Ok(self.environment.lock().unwrap().get(name).cloned())
    }

    /// 【搜索凭据测试】【请求记录】继续执行真实 HTTP 能力检查并记录认证头
    /// @param request 请求；capabilities 为有效授权；allow_writes 为调用权限
    /// @returns 固定查询响应
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        self.http.http(request, capabilities, allow_writes).await
    }
}

/// 【搜索凭据测试】【运行样本】可独立授予或撤销环境能力，不修改包声明
/// @param keys 显式密钥列表；allow_env 是否授予环境读取；host 为隔离宿主
/// @returns 搜索示例运行时
fn runtime(keys: Value, allow_env: bool, host: Arc<CredentialHost>) -> PluginRuntime {
    let mut descriptor = descriptor(json!({"tavily_api_keys":keys}));
    let mut grants = descriptor.grants();
    if !allow_env {
        grants.system.environment.clear();
    }
    descriptor.setting.grants = Some(grants);
    PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host,
    )
    .unwrap()
}

/// 【搜索凭据测试】【显式环境授权】未授权时不读取宿主变量，授权后每次请求读取当前值
/// @returns 无；管理初始化不读取环境，也不把值写入设置
#[tokio::test]
async fn environment_credentials_require_grants_and_are_read_during_calls() {
    let host = Arc::new(CredentialHost::new());
    let denied = runtime(json!(["$env:TAVILY_API_KEY"]), false, host.clone());
    let arguments = json!({"query":"Rust","provider":"tavily"});
    assert!(denied
        .call_tool(TOOL, arguments.clone(), InvocationContext::default())
        .await
        .is_err());
    assert!(host.reads.lock().unwrap().is_empty());
    assert!(host.http.requests.lock().unwrap().is_empty());
    let allowed = runtime(json!(["$env:TAVILY_API_KEY"]), true, host.clone());
    assert!(host.reads.lock().unwrap().is_empty());
    allowed
        .call_tool(TOOL, arguments.clone(), InvocationContext::default())
        .await
        .unwrap();
    host.environment
        .lock()
        .unwrap()
        .insert("TAVILY_API_KEY".into(), "second-key".into());
    allowed
        .call_tool(TOOL, arguments, InvocationContext::default())
        .await
        .unwrap();
    let requests = host.http.requests.lock().unwrap();
    assert_eq!(requests[0].headers["authorization"], "Bearer first-key");
    assert_eq!(requests[1].headers["authorization"], "Bearer second-key");
}

/// 【搜索凭据测试】【凭据顺序】字面密钥优先，授权环境引用及供应商变量依次回退
/// @returns 无；未声明变量不得读取，显式密钥不触发环境访问
#[tokio::test]
async fn credential_priority_preserves_explicit_reference_and_fallback_order() {
    for (keys, expected, reads) in [
        (
            json!([" literal-key ", "$env:TAVILY_API_KEY"]),
            "Bearer literal-key",
            0,
        ),
        (
            json!(["", "$env: TAVILY_API_KEY ", "unused"]),
            "Bearer first-key",
            1,
        ),
        (json!(["$env:UNDECLARED_VARIABLE"]), "Bearer first-key", 1),
        (json!([]), "Bearer first-key", 1),
    ] {
        let host = Arc::new(CredentialHost::new());
        let plugin = runtime(keys, true, host.clone());
        plugin
            .call_tool(
                TOOL,
                json!({"query":"Rust","provider":"tavily"}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            host.http.requests.lock().unwrap()[0].headers["authorization"],
            expected
        );
        assert_eq!(host.reads.lock().unwrap().len(), reads);
    }
}
