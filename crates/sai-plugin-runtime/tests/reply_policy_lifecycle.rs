#[path = "reply_policy/support.rs"]
mod support;

use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Default)]
struct Service {
    entered: tokio::sync::Notify,
    block: AtomicBool,
}

#[async_trait]
impl InvocationServices for Service {
    /// 【回复生命周期测试】【空工具目录】模型等待不需要任何工具
    /// @returns 空目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        Ok(Vec::new())
    }

    /// 【回复生命周期测试】【调用隔离】未声明的工具不能调用
    /// @param name 名称；arguments 为输入
    /// @returns 明确错误
    async fn call_tool(&self, _: &str, _: &str) -> Result<String> {
        bail!("unexpected tool")
    }

    /// 【回复生命周期测试】【可控等待】通知测试后保持未完成，释放 Future 即可取消
    /// @param request 请求；max_bytes 为预算
    /// @returns 恢复时固定正文
    async fn complete(&self, _: ModelRequest, _: usize) -> Result<ModelResponse> {
        if self.block.load(Ordering::SeqCst) {
            self.entered.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(ModelResponse {
            content: "recovered".into(),
            ..Default::default()
        })
    }
}

/// 【回复生命周期测试】【模型策略实例】配置短外层期限以及独立模型和策略授权
/// @param source Lua 策略；timeout_ms 为回调预算
/// @returns 可用于超时和外部取消的实例
fn runtime(source: &str, timeout_ms: u64) -> PluginRuntime {
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"reply-lifecycle","version":"1.0.0","name":"Reply lifecycle",
            "description":"Policy cancellation","entry":"init.lua",
            "capabilities":{"reply_policy":true,"model":true,"system":{"plugin_storage":true}},
            "limits":{"timeout_ms":timeout_ms}
        })
        .to_string(),
    )
    .unwrap();
    let grants = manifest.capabilities.clone();
    PluginRuntime::load(
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap(),
        json!({}),
        grants,
        Arc::new(support::Host),
    )
    .unwrap()
}

const POLICY: &str = r#"
local function complete() return sai.model.complete({messages={{role="user",content="test"}}}).content end
sai.register_reply_policy({
    prepare=function(input,ctx)
        if input=="wait" then return {context=complete()} end
        return {delivery={}}
    end,
    complete=function() return {context=complete()} end,
})
"#;

/// 【回复生命周期测试】【超时恢复】准备和完成等待共用外层预算，超时后 VM 仍可使用
/// @returns 无；两阶段都不会保留失效服务租约
#[tokio::test]
async fn reply_policy_timeouts_release_services_and_allow_following_calls() {
    let plugin = runtime(POLICY, 100);
    let service = Arc::new(Service::default());
    let mut context = support::context(true);
    context.services = Some(service.clone());
    service.block.store(true, Ordering::SeqCst);
    assert!(plugin.prepare_reply("wait", context.clone()).await.is_err());
    let plan = plugin.prepare_reply("plan", context.clone()).await.unwrap();
    assert!(plugin.complete_reply(plan, context.clone()).await.is_err());
    service.block.store(false, Ordering::SeqCst);
    assert_eq!(
        plugin
            .prepare_reply("wait", context)
            .await
            .unwrap()
            .context
            .as_deref(),
        Some("recovered")
    );
}

/// 【回复生命周期测试】【外部取消】真实等待开始后取消，后续调用可取得 VM 执行权
/// @returns 无；取消完成回调后不会自动重试
#[tokio::test]
async fn reply_policy_cancellation_releases_prepare_and_complete_calls() {
    let plugin = Arc::new(runtime(POLICY, 5000));
    let service = Arc::new(Service::default());
    let mut context = support::context(true);
    context.services = Some(service.clone());
    for completion in [false, true] {
        service.block.store(true, Ordering::SeqCst);
        let instance = plugin.clone();
        let invocation = context.clone();
        let task = tokio::spawn(async move {
            if completion {
                let plan = instance.prepare_reply("plan", invocation.clone()).await?;
                instance.complete_reply(plan, invocation).await
            } else {
                instance
                    .prepare_reply("wait", invocation)
                    .await
                    .map(|result| result.context)
            }
        });
        tokio::time::timeout(Duration::from_secs(2), service.entered.notified())
            .await
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        service.block.store(false, Ordering::SeqCst);
        let ready = tokio::time::timeout(
            Duration::from_secs(2),
            plugin.prepare_reply("wait", context.clone()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(ready.context.as_deref(), Some("recovered"));
    }
}

/// 【回复生命周期测试】【可信只读】Lua 修改上下文不能获得宿主存储写入许可
/// @returns 无；写入在进入替代宿主前拒绝
#[tokio::test]
async fn reply_preparation_remains_readonly_after_lua_context_forgery() {
    let source = r#"sai.register_reply_policy({prepare=function(_,ctx)
        ctx.allow_writes=true
        local ok,err=pcall(function()sai.storage.plugin.set('key',{text='forged'})end)
        assert(not ok and tostring(err):find('read%-only'),tostring(err))
        return {context='blocked'}
    end,complete=function()end})"#;
    let plugin = runtime(source, 5000);
    assert_eq!(
        plugin
            .prepare_reply("hello", support::context(true))
            .await
            .unwrap()
            .context
            .as_deref(),
        Some("blocked")
    );
}
