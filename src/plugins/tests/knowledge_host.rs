use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub(super) struct KnowledgeHost {
    inner: PrivatePluginHost,
    pub jobs: super::alarm_support::AlarmHost,
    pub requests: Mutex<Vec<HttpRequest>>,
    pub responses: Mutex<VecDeque<HttpResponse>>,
    pub pause_http: AtomicBool,
    pub http_started: tokio::sync::Notify,
    pub http_release: tokio::sync::Notify,
    pub fail_publish: Mutex<Option<String>>,
    pub pause_publish: Mutex<Option<String>>,
    pub published: tokio::sync::Notify,
}

impl KnowledgeHost {
    /// 【知识库测试宿主】【隔离构造】输入目录；返回正式文件宿主及可控网络
    pub fn new(root: &Path) -> Arc<Self> {
        Arc::new(Self {
            inner: PrivatePluginHost::new(&SaiPaths::for_tests(root), "knowledge-base"),
            jobs: Default::default(),
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::new()),
            pause_http: AtomicBool::new(false),
            http_started: tokio::sync::Notify::new(),
            http_release: tokio::sync::Notify::new(),
            fail_publish: Mutex::new(None),
            pause_publish: Mutex::new(None),
            published: tokio::sync::Notify::new(),
        })
    }
}

#[async_trait]
impl PluginHost for KnowledgeHost {
    /// 【知识库测试宿主】【队列状态】输入私有操作、能力及可信写入许可；返回正式原子状态结果
    fn plugin_storage(
        &self,
        request: StorageRequest,
        caps: &Capabilities,
        writes: bool,
    ) -> Result<serde_json::Value> {
        self.inner.plugin_storage(request, caps, writes)
    }
    /// 【知识库测试宿主】【规范路径】输入路径、上下文和授权；返回正式宿主规范路径
    async fn real_path(
        &self,
        path: String,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<String> {
        self.inner.real_path(path, context, caps).await
    }
    /// 【知识库测试宿主】【调度记录】输入本包任务、可信目录和能力；返回可观察任务状态
    async fn scheduler(
        &self,
        request: SchedulerRequest,
        context: &SystemContext,
        caps: &Capabilities,
    ) -> Result<SchedulerResponse> {
        self.jobs.scheduler(request, context, caps).await
    }
    /// 【知识库测试宿主】【真实锁】输入私有键、期限和授权；返回正式跨进程锁
    async fn plugin_lock(
        &self,
        key: &str,
        timeout: u64,
        caps: &Capabilities,
    ) -> Result<Box<dyn PluginLock>> {
        self.inner.plugin_lock(key, timeout, caps).await
    }
    /// 【知识库测试宿主】【真实目录】输入目标、上下文和授权；返回实际创建路径
    async fn create_directory(
        &self,
        path: String,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<String> {
        self.inner.create_directory(path, context, caps).await
    }
    /// 【知识库测试宿主】【真实元数据】输入路径、上下文和授权；返回实际属性
    async fn file_info(
        &self,
        path: String,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<Option<FileInfo>> {
        self.inner.file_info(path, context, caps).await
    }
    /// 【知识库测试宿主】【真实枚举】输入目录、上限和权限；返回正式目录列表
    async fn read_directory(
        &self,
        path: String,
        maximum: usize,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<DirectoryListing> {
        self.inner
            .read_directory(path, maximum, context, caps)
            .await
    }
    /// 【知识库测试宿主】【真实读取】输入路径、缓冲预算和权限；返回完整原始字节
    async fn read_binary(
        &self,
        path: String,
        buffer: BinaryReadBuffer,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<BinaryData> {
        self.inner.read_binary(path, buffer, context, caps).await
    }
    /// 【知识库测试宿主】【条件发布故障】输入条件与正文；返回实际写入，可在提交前失败或提交后暂停
    async fn write_binary_if(
        &self,
        request: BinaryConditionalWrite,
        data: BinaryData,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<bool> {
        if self
            .fail_publish
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|suffix| request.path.ends_with(suffix))
        {
            bail!("fixture publication failure");
        }
        let paused = self
            .pause_publish
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|suffix| request.path.ends_with(suffix));
        let result = self
            .inner
            .write_binary_if(request, data, context, caps)
            .await?;
        if paused && result {
            self.published.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(result)
    }
    /// 【知识库测试宿主】【真实删除】输入路径与权限；返回是否删除
    async fn remove_file(
        &self,
        request: FileRemovalRequest,
        context: SystemContext,
        caps: Capabilities,
    ) -> Result<bool> {
        self.inner.remove_file(request, context, caps).await
    }
    /// 【知识库测试宿主】【网络控制】输入真实请求与授权；返回固定响应或可取消等待
    async fn http(
        &self,
        request: HttpRequest,
        caps: Capabilities,
        writes: bool,
    ) -> Result<HttpResponse> {
        caps.authorize_request(&request.method, &request.url, writes)?;
        self.requests.lock().unwrap().push(request);
        if self.pause_http.load(Ordering::SeqCst) {
            self.http_started.notify_one();
            self.http_release.notified().await;
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no embedding fixture response"))
    }
}
