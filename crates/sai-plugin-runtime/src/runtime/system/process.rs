use super::{charge, error, result_value};
use crate::host::{PluginHost, ProcessOutput, ProcessRequest};
use crate::runtime::control::CallControl;
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;
use std::time::Duration;

/// 【插件】【进程选项】只允许收窄等待时长和两路输出大小。
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProcessOptions {
    timeout_ms: Option<u64>,
    max_stdout_bytes: Option<usize>,
    max_stderr_bytes: Option<usize>,
}

/// 【插件】【进程绑定】调用完整授权模板，超时或外部取消会释放正在执行的宿主 Future。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为限制；control 为状态
/// @returns 进程接口安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let process = lua.create_table()?;
    process.set(
        "output",
        lua.create_async_function(
            move |lua, (template, parameters, options): (String, Table, Option<Table>)| {
                let (host, capabilities, limits, control) = (
                    host.clone(),
                    capabilities.clone(),
                    limits.clone(),
                    control.clone(),
                );
                async move {
                    let context = control.system_context()?;
                    let parameters = lua.from_value(Value::Table(parameters))?;
                    capabilities
                        .system
                        .process_command(&template, &parameters, context.allow_writes)
                        .map_err(error)?;
                    let options: ProcessOptions = options
                        .map(|table| lua.from_value(Value::Table(table)))
                        .transpose()?
                        .unwrap_or_default();
                    let request = ProcessRequest {
                        template,
                        parameters,
                        timeout_ms: options
                            .timeout_ms
                            .unwrap_or(30_000)
                            .clamp(1, limits.timeout_ms.min(120_000)),
                        max_stdout_bytes: options
                            .max_stdout_bytes
                            .unwrap_or(64 * 1024)
                            .clamp(1, limits.output_bytes / 2),
                        max_stderr_bytes: options
                            .max_stderr_bytes
                            .unwrap_or(16 * 1024)
                            .clamp(1, limits.output_bytes / 2),
                    };
                    let timeout = Duration::from_millis(request.timeout_ms);
                    let stdout_limit = request.max_stdout_bytes;
                    let stderr_limit = request.max_stderr_bytes;
                    charge(&control, limits.system_calls)?;
                    let output = match tokio::time::timeout(
                        timeout,
                        host.process(request, context, capabilities),
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
                            "plugin process output exceeds requested byte limit",
                        ));
                    }
                    result_value(&lua, &output, limits.output_bytes)
                }
            },
        )?,
    )?;
    api.set("process", process)
}
