use crate::paths::SaiPaths;
use crate::plugins::private::PrivatePluginHost;
use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// 【AUR 测试】【固定进程】所有程序调用都由替身处理，文件读取与状态存储使用正式宿主。
pub(super) struct AurHost {
    private: PrivatePluginHost,
    pub scenario: Arc<Scenario>,
}

#[derive(Default)]
pub(super) struct Scenario {
    pub files: Mutex<BTreeMap<String, Vec<u8>>>,
    pub helper: Mutex<Option<String>>,
    pub calls: Mutex<Vec<(String, Value, String, u64)>>,
    pub outcomes: Mutex<BTreeMap<String, ProcessOutput>>,
    pub metadata_error: AtomicBool,
}

impl AurHost {
    /// 【AUR 测试】【独立路径】绑定临时应用目录及固定元数据。
    /// @param paths 隔离应用路径；helper 为可用助手；content 为 PKGBUILD 正文
    /// @returns 不执行真实系统进程的宿主
    pub fn new(paths: &SaiPaths, helper: Option<&str>, content: &str) -> Self {
        let scenario = Scenario::default();
        scenario
            .files
            .lock()
            .unwrap()
            .insert("PKGBUILD".into(), content.as_bytes().to_vec());
        *scenario.helper.lock().unwrap() = helper.map(str::to_string);
        Self {
            private: PrivatePluginHost::new(paths, "package-advisor"),
            scenario: Arc::new(scenario),
        }
    }
}

/// 【AUR 测试】【正常输出】构造有界且成功退出的进程结果。
/// @returns 固定进程输出
pub(super) fn success() -> ProcessOutput {
    ProcessOutput {
        status: Some(0),
        stdout: " done \n".into(),
        stderr: String::new(),
        timed_out: false,
        stdout_truncated: false,
        stderr_truncated: false,
    }
}

#[async_trait]
impl PluginHost for AurHost {
    /// 【AUR 测试】【状态代理】使用正式的会话隔离和原子比较交换。
    /// @param request 操作；session 为会话；capabilities 为权限
    /// @returns 状态结果
    fn storage(
        &self,
        request: StorageRequest,
        session: &str,
        capabilities: &Capabilities,
    ) -> Result<Value> {
        self.private.storage(request, session, capabilities)
    }

    /// 【AUR 测试】【目录代理】保留真实目录约束，只替换网络解压和程序执行。
    /// @param key 目录键；session 为会话；capabilities 为权限
    /// @returns 测试目录句柄
    fn workspace(
        &self,
        key: &str,
        session: &str,
        capabilities: &Capabilities,
    ) -> Result<Arc<dyn PluginWorkspace>> {
        Ok(Arc::new(AurWorkspace {
            inner: self.private.workspace(key, session, capabilities)?,
            scenario: self.scenario.clone(),
        }))
    }

    /// 【AUR 测试】【版本探测】按固定场景返回助手是否可用。
    /// @param request 模板；context 为权限；capabilities 为授权
    /// @returns 固定探测结果
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
        self.scenario.calls.lock().unwrap().push((
            request.template.clone(),
            request.parameters,
            context.workdir,
            request.timeout_ms,
        ));
        let mut output = success();
        output.status = Some(
            if self
                .scenario
                .helper
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|name| request.template == format!("{name}_version"))
            {
                0
            } else {
                1
            },
        );
        Ok(output)
    }

    /// 【AUR 测试】【元数据】返回固定 RPC 正文，不访问互联网。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定响应或显式失败
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        capabilities.authorize_request(&request.method, &request.url, allow_writes)?;
        if self.scenario.metadata_error.load(Ordering::SeqCst) {
            anyhow::bail!("fixture metadata failure");
        }
        Ok(HttpResponse { status: 200, headers: Default::default(), text: json!({"resultcount":1,"results":[{"Name":"demo","URLPath":"/snapshot/demo.tar.gz"}]}).to_string() })
    }
}

struct AurWorkspace {
    inner: Arc<dyn PluginWorkspace>,
    scenario: Arc<Scenario>,
}

impl AurWorkspace {
    /// 【AUR 测试】【构建文件样本】只向临时工作目录写入固定文件。
    /// @param prefix 构建文件的相对子目录
    /// @returns 写入结果
    fn populate(&self, prefix: &str) -> Result<()> {
        let root = std::path::PathBuf::from(self.inner.path()).join(prefix);
        for (name, bytes) in self.scenario.files.lock().unwrap().iter() {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, bytes)?;
        }
        Ok(())
    }
}

#[async_trait]
impl PluginWorkspace for AurWorkspace {
    /// 【AUR 测试】【显示路径】读取真实隔离目录路径。
    /// @returns 显示路径
    fn path(&self) -> String {
        self.inner.path()
    }
    /// 【AUR 测试】【文件读取】调用正式读取接口。
    /// @param request 请求
    /// @returns 文本结果
    async fn read_text(&self, request: FileReadRequest) -> Result<FileText> {
        self.inner.read_text(request).await
    }
    /// 【AUR 测试】【目录读取】调用正式目录接口。
    /// @param path 相对路径；max_entries 为上限
    /// @returns 条目列表
    async fn read_directory(&self, path: String, max_entries: usize) -> Result<DirectoryListing> {
        self.inner.read_directory(path, max_entries).await
    }
    /// 【AUR 测试】【属性读取】调用正式属性接口。
    /// @param path 相对路径
    /// @returns 属性
    async fn file_info(&self, path: String) -> Result<Option<FileInfo>> {
        self.inner.file_info(path).await
    }
    /// 【AUR 测试】【归档替身】验证来源后生成固定快照，安全解压另有宿主测试。
    /// @param request 请求；capabilities 为授权
    /// @returns 固定快照生成结果
    async fn extract_archive(
        &self,
        request: ArchiveRequest,
        capabilities: Capabilities,
    ) -> Result<()> {
        capabilities.authorize_url(&request.url)?;
        self.populate(&format!("{}/demo", request.destination))
    }
    /// 【AUR 测试】【安装替身】记录模板和时限，只生成假构建产物，不调用系统包管理器。
    /// @param request 模板；directory 为相对目录；capabilities 为授权；allow_writes 为权限
    /// @returns 固定步骤结果
    async fn process(
        &self,
        request: ProcessRequest,
        directory: String,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<ProcessOutput> {
        capabilities.system.process_command(
            &request.template,
            &request.parameters,
            allow_writes,
        )?;
        assert!(capabilities.system.processes[&request.template].workspace);
        self.scenario.calls.lock().unwrap().push((
            request.template.clone(),
            request.parameters.clone(),
            directory.clone(),
            request.timeout_ms,
        ));
        if request.template.ends_with("_fetch") {
            self.populate("demo")?;
        }
        if request.template == "makepkg" {
            let root = std::path::PathBuf::from(self.path()).join(directory);
            std::fs::write(root.join("demo-1-1-any.pkg.tar.zst"), "fake package")?;
            std::fs::write(root.join("000.pkg.tar.zst.sig"), "fake signature")?;
        }
        Ok(self
            .scenario
            .outcomes
            .lock()
            .unwrap()
            .get(&request.template)
            .cloned()
            .unwrap_or_else(success))
    }
}

/// 【AUR 测试】【实际源码】在固定进程测试中模拟 Linux 平台，其他源码和清单保持正式内容。
/// @returns 只使用假安装进程的包
pub(super) fn package() -> PluginPackage {
    let package = super::query_support::package("package-advisor");
    let mut sources = package.sources().clone();
    sources
        .get_mut("init.lua")
        .unwrap()
        .insert_str(0, "sai.system.platform = 'linux'\n");
    PluginPackage::new(package.manifest, sources).unwrap()
}

/// 【AUR 测试】【运行实例】固定源代码与完整声明，重复调用共享会话存储。
/// @param host 测试宿主
/// @returns 实际 Lua 运行时
pub(super) fn runtime(host: Arc<AurHost>) -> PluginRuntime {
    let package = package();
    let grants = package.manifest.capabilities.clone();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【AUR 测试】【可信上下文】构造明确会话、操作及写入权限。
/// @param session 会话；operation 为用户操作；writes 为写入许可
/// @returns 调用上下文
pub(super) fn context(session: &str, operation: &str, writes: bool) -> InvocationContext {
    InvocationContext {
        session_id: session.into(),
        operation_id: operation.into(),
        allow_writes: writes,
        ..Default::default()
    }
}

/// 【AUR 测试】【JSON 结果】通过真实工具运行并解析结果。
/// @param runtime 实例；name 为工具名；args 为参数；ctx 为可信上下文
/// @returns JSON 结果
pub(super) async fn call(
    runtime: &PluginRuntime,
    name: &str,
    args: Value,
    ctx: InvocationContext,
) -> Value {
    serde_json::from_str(&runtime.call_tool(name, args, ctx).await.unwrap()).unwrap()
}
