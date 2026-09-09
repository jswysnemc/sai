use super::WorkspaceHandle;
use crate::host::{ProcessOutput, ProcessRequest};
use crate::runtime::system::{error, result_value};
use mlua::{LuaSerdeExt, Table, UserDataMethods, Value};

#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    directory: Option<String>,
    timeout_ms: Option<u64>,
    max_stdout_bytes: Option<usize>,
    max_stderr_bytes: Option<usize>,
}

/// 【插件】【目录进程绑定】固定程序、参数和工作目录类型都必须匹配授权模板。
/// @param methods 私有工作目录的方法表
/// @returns 无；超时释放进程 Future，由宿主回收进程树
pub(super) fn install<M: UserDataMethods<WorkspaceHandle>>(methods: &mut M) {
    methods.add_async_method(
        "process",
        |lua, this, (template, parameters, options): (String, Table, Option<Table>)| async move {
            let workspace = this.current()?;
            let allow_writes = this.control.system_context()?.allow_writes;
            let parameters = lua.from_value(Value::Table(parameters))?;
            this.capabilities
                .system
                .process_command(&template, &parameters, allow_writes)
                .map_err(error)?;
            if !this.capabilities.system.processes[&template].workspace {
                return Err(mlua::Error::runtime(
                    "process template does not allow a private workspace",
                ));
            }
            let options: Options = options
                .map(|table| lua.from_value(Value::Table(table)))
                .transpose()?
                .unwrap_or_default();
            let directory = options.directory.unwrap_or_else(|| ".".into());
            this.charge_path(&directory)?;
            let request = ProcessRequest {
                template,
                parameters,
                timeout_ms: options
                    .timeout_ms
                    .unwrap_or(30_000)
                    .clamp(1, this.limits.timeout_ms.min(1_800_000)),
                max_stdout_bytes: options
                    .max_stdout_bytes
                    .unwrap_or(64 * 1024)
                    .clamp(1, this.limits.output_bytes / 2),
                max_stderr_bytes: options
                    .max_stderr_bytes
                    .unwrap_or(16 * 1024)
                    .clamp(1, this.limits.output_bytes / 2),
            };
            let timeout = std::time::Duration::from_millis(request.timeout_ms);
            let (stdout_limit, stderr_limit) = (request.max_stdout_bytes, request.max_stderr_bytes);
            let output = match tokio::time::timeout(
                timeout,
                workspace.process(request, directory, this.capabilities.clone(), allow_writes),
            )
            .await
            {
                Ok(result) => result.map_err(error)?,
                Err(_) => ProcessOutput {
                    status: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    timed_out: true,
                    stdout_truncated: false,
                    stderr_truncated: false,
                },
            };
            if output.stdout.len() > stdout_limit || output.stderr.len() > stderr_limit {
                return Err(mlua::Error::runtime(
                    "workspace process output exceeds requested byte limit",
                ));
            }
            result_value(&lua, &output, this.limits.output_bytes)
        },
    );
}
