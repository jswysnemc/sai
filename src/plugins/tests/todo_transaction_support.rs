use super::todo_support::package;
use crate::plugins::private::PrivatePluginHost;
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, PluginRuntime};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};

/// 【待办事务测试】【故障宿主】保留正式文件操作，仅在提交边界加入可观察故障
pub(super) struct TransactionHost {
    pub inner: Arc<PrivatePluginHost>,
    pub replacement: Mutex<Option<Value>>,
    pub conflicts: AtomicUsize,
    pub fail: AtomicUsize,
    pub published: tokio::sync::Notify,
    pub pause: Mutex<bool>,
    pub resumed: Condvar,
    pub exchanges: AtomicUsize,
}

impl TransactionHost {
    /// 【待办事务测试】【故障装配】包装已绑定内置来源的正式宿主
    /// @param inner 正式宿主
    /// @returns 默认无故障的包装实例
    pub fn new(inner: Arc<PrivatePluginHost>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            replacement: Mutex::new(None),
            conflicts: AtomicUsize::new(0),
            fail: AtomicUsize::new(0),
            published: tokio::sync::Notify::new(),
            pause: Mutex::new(false),
            resumed: Condvar::new(),
            exchanges: AtomicUsize::new(0),
        })
    }

    /// 【待办事务测试】【恢复提交】释放已经完成原子发布的同步宿主调用
    /// @returns 无
    pub fn resume(&self) {
        *self.pause.lock().unwrap() = false;
        self.resumed.notify_all();
    }
}

#[async_trait]
impl PluginHost for TransactionHost {
    /// 【待办事务测试】【网络隔离】待办包不允许发出网络请求
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【待办事务测试】【提交边界】注入一次并发修改、冲突、写入失败或发布后暂停
    /// @param request 请求；session 为可信会话；capabilities 为真实授权
    /// @returns 正式存储结果或指定故障
    fn storage(
        &self,
        request: StorageRequest,
        session: &str,
        capabilities: &Capabilities,
    ) -> Result<Value> {
        let exchange = matches!(request, StorageRequest::CompareExchange { .. });
        if exchange {
            self.exchanges.fetch_add(1, Ordering::SeqCst);
            if let Some(value) = self.replacement.lock().unwrap().take() {
                self.inner.storage(
                    StorageRequest::Set {
                        key: "plan".into(),
                        value,
                    },
                    session,
                    capabilities,
                )?;
            }
            if self
                .conflicts
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
            {
                return Ok(json!(false));
            }
            if self.fail.load(Ordering::SeqCst) != 0 {
                bail!("injected publication failure");
            }
        }
        let result = self.inner.storage(request, session, capabilities)?;
        if exchange && result == true {
            let mut paused = self.pause.lock().unwrap();
            if *paused {
                self.published.notify_one();
                while *paused {
                    paused = self.resumed.wait(paused).unwrap();
                }
            }
        }
        Ok(result)
    }
}

/// 【待办事务测试】【原业务实例】使用不修改源码的实际待办包
/// @param host 带提交故障的正式宿主
/// @returns 独立运行时
pub(super) fn runtime(host: Arc<TransactionHost>) -> PluginRuntime {
    let package = package();
    let grants = package.manifest.capabilities.clone();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}
