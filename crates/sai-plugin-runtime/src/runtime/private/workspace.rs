use crate::host::{
    validate_storage_key, validate_workspace_path, ArchiveRequest, FileReadRequest, PluginHost,
    PluginWorkspace,
};
use crate::runtime::{
    control::CallControl,
    system::{charge, error, result_value},
};
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, UserData, UserDataMethods, Value};
use std::sync::{atomic::Ordering, Arc};

#[path = "workspace_process.rs"]
mod process;

/// 【插件】【目录租约】Lua 只持有索引，宿主目录和锁在回调结束时统一释放。
struct WorkspaceHandle {
    index: usize,
    generation: u64,
    control: Arc<CallControl>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
}

#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReadOptions {
    max_bytes: Option<usize>,
    lossy: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveOptions {
    url: String,
    destination: String,
    max_bytes: Option<usize>,
    max_unpacked_bytes: Option<u64>,
    max_entries: Option<usize>,
    timeout_ms: Option<u64>,
}

impl WorkspaceHandle {
    /// 【插件】【目录有效性】拒绝旧回调保存的句柄，不能让 Lua 延长锁与文件资源寿命。
    /// @returns 当前回调拥有的真实目录
    fn current(&self) -> mlua::Result<Arc<dyn PluginWorkspace>> {
        self.control.private_session(true)?;
        if self.generation != self.control.generation.load(Ordering::Acquire) {
            return Err(mlua::Error::runtime("plugin workspace handle has expired"));
        }
        self.control
            .workspaces
            .lock()
            .map_err(|_| mlua::Error::runtime("plugin workspace lock poisoned"))?
            .get(self.index)
            .cloned()
            .ok_or_else(|| mlua::Error::runtime("plugin workspace handle has expired"))
    }

    /// 【插件】【目录预算】验证相对路径，再消费本次系统调用额度。
    /// @param path 相对文件或目录路径
    /// @returns 校验和预算均通过时成功
    fn charge_path(&self, path: &str) -> mlua::Result<()> {
        validate_workspace_path(path).map_err(error)?;
        charge(&self.control, self.limits.system_calls)
    }
}

impl UserData for WorkspaceHandle {
    /// 【插件】【目录方法】每个方法都重新校验租约、相对路径与结果大小。
    /// @param methods 当前句柄类型的方法注册表
    /// @returns 无
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("path", |_, this, ()| Ok(this.current()?.path()));
        methods.add_async_method(
            "read_text",
            |lua, this, (path, options): (String, Option<Table>)| async move {
                let workspace = this.current()?;
                let options: ReadOptions = options
                    .map(|table| lua.from_value(Value::Table(table)))
                    .transpose()?
                    .unwrap_or_default();
                let request = FileReadRequest {
                    path,
                    max_bytes: options
                        .max_bytes
                        .unwrap_or(64 * 1024)
                        .clamp(1, this.limits.output_bytes / 2),
                    lossy: options.lossy,
                };
                this.charge_path(&request.path)?;
                let limit = request.max_bytes;
                let result = workspace.read_text(request).await.map_err(error)?;
                if result.text.len() > limit {
                    return Err(mlua::Error::runtime(
                        "workspace text exceeds requested byte limit",
                    ));
                }
                result_value(&lua, &result, this.limits.output_bytes)
            },
        );
        methods.add_async_method(
            "read_dir",
            |lua, this, (path, maximum): (String, Option<usize>)| async move {
                let workspace = this.current()?;
                let maximum = maximum.unwrap_or(256).clamp(1, 1024);
                this.charge_path(&path)?;
                let result = workspace
                    .read_directory(path, maximum)
                    .await
                    .map_err(error)?;
                if result.entries.len() > maximum {
                    return Err(mlua::Error::runtime(
                        "workspace directory exceeds requested entry limit",
                    ));
                }
                result_value(&lua, &result, this.limits.output_bytes)
            },
        );
        methods.add_async_method("stat", |lua, this, path: String| async move {
            let workspace = this.current()?;
            this.charge_path(&path)?;
            let result = workspace.file_info(path).await.map_err(error)?;
            result_value(&lua, &result, this.limits.output_bytes)
        });
        methods.add_async_method("extract_tar_gz", |lua, this, options: Table| async move {
            let workspace = this.current()?;
            let options: ArchiveOptions = lua.from_value(Value::Table(options))?;
            this.capabilities
                .authorize_request("GET", &options.url, false)
                .map_err(error)?;
            this.charge_path(&options.destination)?;
            let request = ArchiveRequest {
                url: options.url,
                destination: options.destination,
                max_bytes: options
                    .max_bytes
                    .unwrap_or(8 * 1024 * 1024)
                    .clamp(1, 8 * 1024 * 1024),
                max_unpacked_bytes: options
                    .max_unpacked_bytes
                    .unwrap_or(32 * 1024 * 1024)
                    .clamp(1, 64 * 1024 * 1024),
                max_entries: options.max_entries.unwrap_or(1024).clamp(1, 4096),
                timeout_ms: options
                    .timeout_ms
                    .unwrap_or(30_000)
                    .clamp(1, this.limits.timeout_ms.min(120_000)),
            };
            tokio::time::timeout(
                std::time::Duration::from_millis(request.timeout_ms),
                workspace.extract_archive(request, this.capabilities.clone()),
            )
            .await
            .map_err(|_| mlua::Error::runtime("plugin archive request timed out"))?
            .map_err(error)?;
            Ok(true)
        });
        process::install(methods);
    }
}

/// 【插件】【目录创建绑定】只有显式授权的工具或命令可创建私有工作目录。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为预算；control 为调用状态
/// @returns 工作目录入口绑定结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    // 【插件】【句柄初始化】1. 在封闭 coroutine 全局入口前构造异步方法元表，不创建工作目录
    let _ = lua.create_userdata(WorkspaceHandle {
        index: usize::MAX,
        generation: 0,
        control: control.clone(),
        capabilities: capabilities.clone(),
        limits: limits.clone(),
    })?;
    let workspace = lua.create_table()?;
    workspace.set(
        "open",
        lua.create_function(move |lua, key: String| {
            let session = control.private_session(true)?;
            if !capabilities.system.workspace {
                return Err(mlua::Error::runtime(
                    "plugin private workspace is not allowed",
                ));
            }
            validate_storage_key(&key).map_err(error)?;
            let mut workspaces = control
                .workspaces
                .lock()
                .map_err(|_| mlua::Error::runtime("plugin workspace lock poisoned"))?;
            if workspaces.len() >= 4 {
                return Err(mlua::Error::runtime(
                    "plugin workspace count exceeds 4 per callback",
                ));
            }
            charge(&control, limits.system_calls)?;
            let value = host
                .workspace(&key, &session, &capabilities)
                .map_err(error)?;
            let index = workspaces.len();
            workspaces.push(value);
            lua.create_userdata(WorkspaceHandle {
                index,
                generation: control.generation.load(Ordering::Acquire),
                control: control.clone(),
                capabilities: capabilities.clone(),
                limits: limits.clone(),
            })
        })?,
    )?;
    api.set("workspace", workspace)
}
