mod common;

use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{PluginHost, SystemContext};
use sai_plugin_runtime::{
    Capabilities, ExecutionLimits, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct PathHost {
    calls: Mutex<Vec<SystemContext>>,
    output: Option<String>,
}

#[async_trait]
impl PluginHost for PathHost {
    /// 【路径解析测试】【网络隔离】路径解析不应触发 HTTP。
    /// @param request 请求；capabilities 为授权；allow_writes 为写入权限
    /// @returns 固定拒绝
    async fn http(
        &self,
        _request: sai_plugin_runtime::host::HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<sai_plugin_runtime::host::HttpResponse> {
        anyhow::bail!("path fixture must not access the network")
    }

    /// 【路径解析测试】【宿主记录】捕获可信目录，提供固定绝对路径。
    /// @param path 请求；context 为可信目录；capabilities 为授权
    /// @returns 固定路径或模拟失败
    async fn real_path(
        &self,
        path: String,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<String> {
        capabilities.system.check_read_request(&path)?;
        self.calls.lock().unwrap().push(context);
        if path == "fail" {
            anyhow::bail!("fixture resolution failed");
        }
        Ok(self
            .output
            .clone()
            .unwrap_or_else(|| absolute_path("resolved")))
    }
}

/// 【路径解析测试】【平台路径】构造当前平台可识别的绝对路径。
/// @param name 末级名称
/// @returns 绝对路径文本
fn absolute_path(name: &str) -> String {
    if cfg!(windows) {
        format!("C:/fixture/{name}")
    } else {
        format!("/fixture/{name}")
    }
}

/// 【路径解析测试】【运行时】使用独立能力和预算加载探测源码。
/// @param source 源码；granted 为读取授权；limits 为预算；host 为测试宿主
/// @returns 可执行运行时
fn runtime(
    source: &str,
    granted: bool,
    limits: ExecutionLimits,
    host: Arc<dyn PluginHost>,
) -> PluginRuntime {
    let mut manifest = PluginManifest::parse(&json!({
        "api_version":1,"id":"path-fixture","version":"1.0.0","name":"Path fixture",
        "description":"Path contract","entry":"init.lua","capabilities":{"system":{"read_paths":["."]}}
    }).to_string()).unwrap();
    manifest.limits = limits;
    let grants = if granted {
        manifest.capabilities.clone()
    } else {
        Capabilities::default()
    };
    PluginRuntime::load(
        PluginPackage::new(
            manifest,
            BTreeMap::from([("init.lua".into(), source.into())]),
        )
        .unwrap(),
        json!({}),
        grants,
        host,
    )
    .unwrap()
}

const SOURCE: &str = r#"
    sai.register_tool({name="resolve",description="Resolve",parameters={type="object"},execute=function(args,ctx)
        ctx.workdir="/forged"
        return sai.fs.realpath(args.path)
    end})
"#;

/// 【路径解析测试】【加载隔离】接口可在加载时取得，但不能在初始化期间解析宿主路径。
/// @returns 无
#[test]
fn realpath_api_is_available_without_initialization_io() {
    common::runtime(
        r#"
        assert(type(sai.fs.realpath) == "function", "realpath API is missing")
        assert(not pcall(sai.fs.realpath, "."))
    "#,
    );
}

/// 【路径解析测试】【权限与参数】缺失授权及非法输入在宿主前拒绝，公开字段不能覆盖工作目录。
/// @returns 无
#[tokio::test]
async fn realpath_validates_capabilities_inputs_and_trusted_context() {
    let host = Arc::new(PathHost::default());
    let plugin = runtime(SOURCE, true, Default::default(), host.clone());
    for path in [
        json!(false),
        json!(42),
        json!({}),
        json!(""),
        json!("../outside"),
        json!("x\u{0000}y"),
    ] {
        assert!(plugin
            .call_tool("resolve", json!({"path":path}), Default::default())
            .await
            .is_err());
    }
    assert!(host.calls.lock().unwrap().is_empty());
    let denied = runtime(SOURCE, false, Default::default(), host.clone());
    assert!(denied
        .call_tool("resolve", json!({"path":"audio.wav"}), Default::default())
        .await
        .is_err());
    assert!(host.calls.lock().unwrap().is_empty());
    let context = InvocationContext {
        workdir: absolute_path("trusted"),
        ..Default::default()
    };
    assert_eq!(
        plugin
            .call_tool("resolve", json!({"path":"audio.wav"}), context.clone())
            .await
            .unwrap(),
        absolute_path("resolved")
    );
    assert_eq!(host.calls.lock().unwrap()[0].workdir, context.workdir);
}

/// 【路径解析测试】【预算与结果】宿主错误计费，非法参数不计费，下一回调恢复预算。
/// @returns 无
#[tokio::test]
async fn realpath_shares_system_budget_and_rejects_bad_host_output() {
    let host = Arc::new(PathHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name="check",description="Check",parameters={type="object"},execute=function()
            assert(not pcall(sai.fs.realpath, 3))
            assert(not pcall(sai.fs.realpath, "fail"))
            local result=sai.fs.realpath("audio.wav")
            assert(not pcall(sai.fs.realpath, "audio.wav"))
            return result
        end})
    "#,
        true,
        ExecutionLimits {
            system_calls: 2,
            ..Default::default()
        },
        host.clone(),
    );
    for count in [2, 4] {
        assert_eq!(
            plugin
                .call_tool("check", json!({}), Default::default())
                .await
                .unwrap(),
            absolute_path("resolved")
        );
        assert_eq!(host.calls.lock().unwrap().len(), count);
    }
    for output in ["relative".to_string(), absolute_path(&"x".repeat(2000))] {
        let plugin = runtime(
            SOURCE,
            true,
            ExecutionLimits {
                output_bytes: 1024,
                ..Default::default()
            },
            Arc::new(PathHost {
                output: Some(output),
                ..Default::default()
            }),
        );
        assert!(plugin
            .call_tool("resolve", json!({"path":"audio.wav"}), Default::default())
            .await
            .is_err());
    }
    let plugin = runtime(
        SOURCE,
        true,
        Default::default(),
        Arc::new(common::RecordingHost::default()),
    );
    assert!(format!(
        "{:#}",
        plugin
            .call_tool("resolve", json!({"path":"audio.wav"}), Default::default())
            .await
            .unwrap_err()
    )
    .contains("unavailable"));
}
