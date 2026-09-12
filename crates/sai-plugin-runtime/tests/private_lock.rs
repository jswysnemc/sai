use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, Capabilities, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Default)]
struct Host {
    acquired: Arc<AtomicUsize>,
    released: Arc<AtomicUsize>,
}
struct Lease(Arc<AtomicUsize>);
impl PluginLock for Lease {}
impl Drop for Lease {
    /// 【互斥测试】【释放记录】无参数；归还租约时更新观察计数
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[async_trait]
impl PluginHost for Host {
    /// 【互斥测试】【租约记录】输入键、期限和能力；返回可观察的租约
    async fn plugin_lock(
        &self,
        key: &str,
        timeout: u64,
        capabilities: &Capabilities,
    ) -> Result<Box<dyn PluginLock>> {
        assert!(capabilities.system.plugin_storage);
        assert!((1..=600_000).contains(&timeout));
        validate_storage_key(key)?;
        self.acquired.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Lease(self.released.clone())))
    }
    /// 【互斥测试】【网络拒绝】输入请求和能力；返回固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }
}

/// 【互斥测试】【实例装配】输入工具正文、授权和限制；返回实例和观察宿主
fn fixture(body: &str, allowed: bool, limits: Value) -> (PluginRuntime, Arc<Host>) {
    let host = Arc::new(Host::default());
    let manifest=PluginManifest::parse(&json!({"api_version":1,"id":"lock-test","version":"1.0.0","name":"Lock test","description":"Scoped locks","entry":"init.lua","capabilities":{"system":{"plugin_storage":true}},"limits":limits}).to_string()).unwrap();
    let mut grants = manifest.capabilities.clone();
    grants.system.plugin_storage = allowed;
    let source=format!("sai.register_tool({{name='probe',description='Lock test',parameters={{type='object'}},execute=function(args,ctx) {body} end}})");
    let package = PluginPackage::new(manifest, [("init.lua".into(), source)].into()).unwrap();
    (
        PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap(),
        host,
    )
}

/// 【互斥测试】【返回和异常】只读锁保留多个返回值，异常不会泄漏租约
/// @returns 无；两次取得的锁全部释放
#[tokio::test]
async fn private_lock_preserves_returns_and_releases_after_error() {
    let (runtime, host) = fixture(
        r#"
        local a,b,c=sai.storage.plugin.with_lock('a',function() return 12,nil,'value' end)
        assert(a==12 and b==nil and c=='value')
        local ok=pcall(sai.storage.plugin.with_lock,'a',function() error('fixture failure') end)
        assert(not ok)
        return not ctx.allow_writes
    "#,
        true,
        json!({}),
    );
    assert_eq!(
        runtime
            .call_tool("probe", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "true"
    );
    assert_eq!(host.acquired.load(Ordering::SeqCst), 2);
    assert_eq!(host.released.load(Ordering::SeqCst), 2);
}

/// 【互斥测试】【输入与递归】非法键、未知选项和逆序嵌套不能进入宿主
/// @returns 无；同键和反序锁失败后合法调用仍然可用
#[tokio::test]
async fn private_lock_rejects_invalid_and_out_of_order_requests() {
    let (runtime, host) = fixture(
        r#"
        for _,args in ipairs({{1,function()end},{'',function()end},{'a',1},{'a',function()end,{extra=true}},{'a',function()end,{timeout_ms=0}}}) do
            assert(not pcall(sai.storage.plugin.with_lock,table.unpack(args)))
        end
        sai.storage.plugin.with_lock('b',function()
            assert(not pcall(sai.storage.plugin.with_lock,'b',function()end))
            assert(not pcall(sai.storage.plugin.with_lock,'a',function()end))
            sai.storage.plugin.with_lock('c',function()end)
        end)
        return true
    "#,
        true,
        json!({}),
    );
    runtime
        .call_tool("probe", json!({}), InvocationContext::default())
        .await
        .unwrap();
    assert_eq!(host.acquired.load(Ordering::SeqCst), 2);
    assert_eq!(host.released.load(Ordering::SeqCst), 2);
}

/// 【互斥测试】【能力撤销】私有状态授权缺失时不能创建宿主锁
/// @returns 无；Lua 修改公开上下文无效
#[tokio::test]
async fn private_lock_requires_independent_plugin_storage_grant() {
    let (runtime, host) = fixture(
        "ctx.allow_writes=true; return sai.storage.plugin.with_lock('a',function()return true end)",
        false,
        json!({}),
    );
    let error = runtime
        .call_tool("probe", json!({}), InvocationContext::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("plugin storage is not allowed"));
    assert_eq!(host.acquired.load(Ordering::SeqCst), 0);
}

/// 【互斥测试】【执行期限】死循环终止后归还锁，系统次数预算同样限制互斥调用
/// @returns 无；执行失败时租约没有遗留
#[tokio::test]
async fn private_lock_releases_on_instruction_exhaustion_and_charges_budget() {
    let (runtime, host) = fixture(
        "return sai.storage.plugin.with_lock('a',function()while true do end end)",
        true,
        json!({"instructions":10000}),
    );
    assert!(runtime
        .call_tool("probe", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert_eq!(host.acquired.load(Ordering::SeqCst), 1);
    assert_eq!(host.released.load(Ordering::SeqCst), 1);
    let (runtime, host) = fixture(
        "for i=1,2 do sai.storage.plugin.with_lock('a',function()end) end",
        true,
        json!({"system_calls":1}),
    );
    assert!(runtime
        .call_tool("probe", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert_eq!(host.acquired.load(Ordering::SeqCst), 1);
    assert_eq!(host.released.load(Ordering::SeqCst), 1);
}
