use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{
    DirectoryEntry, DirectoryListing, FileInfo, FileReadRequest, FileText, HttpRequest,
    HttpResponse, PluginHost, ProcessOutput, ProcessRequest, SystemContext,
};
use sai_plugin_runtime::{Capabilities, ExecutionLimits, PluginRuntime};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Default)]
pub struct SystemHost {
    pub calls: Mutex<Vec<(String, SystemContext)>>,
    pub oversized: AtomicBool,
    pub blocking: AtomicBool,
    pub active: AtomicUsize,
    pub entered: Notify,
    pub released: Notify,
}

struct Flight<'a>(&'a SystemHost);

impl Drop for Flight<'_> {
    /// 【系统接口测试】【调用结束】记录宿主 Future 是否已随超时或取消释放。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

impl SystemHost {
    /// 【系统接口测试】【记录调用】保存真实上下文，供信任边界断言使用。
    /// @param name 调用名称；context 为运行时传入的上下文
    /// @returns 无
    fn record(&self, name: &str, context: SystemContext) {
        self.calls.lock().unwrap().push((name.into(), context));
    }
}

#[async_trait]
impl PluginHost for SystemHost {
    /// 【系统接口测试】【环境值】返回固定字符串，避免修改测试进程的全局环境。
    /// @param name 精确变量名；capabilities 为授权
    /// @returns 可观察的固定结果
    fn environment(&self, name: &str, capabilities: &Capabilities) -> Result<Option<String>> {
        capabilities.system.authorize_environment(name)?;
        self.record("environment", SystemContext::default());
        Ok(Some("value".into()))
    }

    /// 【系统接口测试】【文件正文】可故意超过请求限制，验证运行时再次检查宿主结果。
    /// @param request 读取要求；context 为可信目录；capabilities 为授权
    /// @returns 固定正文及截断信息
    async fn read_text(
        &self,
        request: FileReadRequest,
        context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<FileText> {
        self.record("read_text", context);
        let extra = usize::from(self.oversized.load(Ordering::SeqCst));
        Ok(FileText {
            text: "x".repeat(request.max_bytes.min(8) + extra),
            truncated: false,
        })
    }

    /// 【系统接口测试】【目录条目】按请求数量产生条目，并可模拟超限宿主。
    /// @param path 目录；max_entries 为条数；context 为可信目录；capabilities 为授权
    /// @returns 固定目录列表
    async fn read_directory(
        &self,
        _path: String,
        max_entries: usize,
        context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<DirectoryListing> {
        self.record("read_directory", context);
        let count = max_entries.min(8) + usize::from(self.oversized.load(Ordering::SeqCst));
        Ok(DirectoryListing {
            entries: (0..count)
                .map(|n| DirectoryEntry {
                    name: n.to_string(),
                    path: n.to_string(),
                    is_file: true,
                    is_dir: false,
                })
                .collect(),
            truncated: false,
        })
    }

    /// 【系统接口测试】【文件属性】记录属性访问，返回缺失结果。
    /// @param path 路径；context 为可信目录；capabilities 为授权
    /// @returns None
    async fn file_info(
        &self,
        _path: String,
        context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        self.record("file_info", context);
        Ok(None)
    }

    /// 【系统接口测试】【进程替身】记录可信权限并可等待取消，不执行实际程序。
    /// @param request 模板参数；context 为可信权限；capabilities 为授权
    /// @returns 固定输出或等待中的 Future
    async fn process(
        &self,
        request: ProcessRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<ProcessOutput> {
        capabilities.system.process_command(
            &request.template,
            &request.parameters,
            context.allow_writes,
        )?;
        self.record("process", context);
        self.active.fetch_add(1, Ordering::SeqCst);
        let _flight = Flight(self);
        self.entered.notify_one();
        if self.blocking.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        let extra = usize::from(self.oversized.load(Ordering::SeqCst));
        Ok(ProcessOutput {
            status: Some(0),
            stdout: "x".repeat(request.max_stdout_bytes.min(8) + extra),
            stderr: "x".repeat(request.max_stderr_bytes.min(8) + extra),
            timed_out: false,
            stdout_truncated: false,
            stderr_truncated: false,
        })
    }

    /// 【系统接口测试】【禁止网络】系统测试不能意外访问网络。
    /// @param request HTTP 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 明确错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        anyhow::bail!("HTTP is unavailable in system tests")
    }
}

/// 【系统接口测试】【授权样本】提供环境、路径和读写各一个进程模板。
/// @returns 已通过真实清单格式解析的能力集合
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system": {
        "read_paths":["."], "environment":["LANG"], "processes": {
            "read": {"program":"read-fixture", "read_only":true},
            "write": {"program":"write-fixture"}
        }
    }}))
    .unwrap()
}

/// 【系统接口测试】【运行实例】用真实 Lua 运行时加载可调预算与授权的系统样本。
/// @param source 脚本；host 为观察宿主；granted 为授权；change 为限制调整
/// @returns 可调用的插件
pub fn runtime(
    source: &str,
    host: Arc<SystemHost>,
    granted: Capabilities,
    change: impl FnOnce(&mut ExecutionLimits),
) -> PluginRuntime {
    let mut package = super::package(source);
    package.manifest.capabilities = capabilities();
    change(&mut package.manifest.limits);
    PluginRuntime::load(package, json!({}), granted, host).unwrap()
}
