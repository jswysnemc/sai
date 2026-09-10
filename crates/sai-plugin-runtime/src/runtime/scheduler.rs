use super::{
    budget,
    control::CallControl,
    system::{charge, error, result_value},
};
use crate::host::{
    PluginHost, ScheduleListOptions, ScheduleRequest, SchedulerRequest, SchedulerResponse,
};
use crate::{Capabilities, ExecutionLimits};
use mlua::{FromLua, Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;

/// 【插件调度】【Lua 绑定】安装独立调度接口，权限与目录始终来自 Rust 调用状态。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为限制；control 为状态
/// @returns 绑定结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let scheduler = lua.create_table()?;
    for action in ["schedule", "list", "get", "cancel", "resume"] {
        let (host, capabilities, limits, control) = (
            host.clone(),
            capabilities.clone(),
            limits.clone(),
            control.clone(),
        );
        scheduler.set(
            action,
            lua.create_async_function(move |lua, input: Value| {
                let (host, capabilities, limits, control) = (
                    host.clone(),
                    capabilities.clone(),
                    limits.clone(),
                    control.clone(),
                );
                async move {
                    // 1. 【插件调度】【可信上下文】初始化拒绝 I/O，公开 Lua 字段不能扩大权限
                    let context = control.system_context()?;
                    let request = match action {
                        "schedule" => {
                            SchedulerRequest::Schedule(lua.from_value::<ScheduleRequest>(input)?)
                        }
                        "list" => {
                            let options = if matches!(input, Value::Nil) {
                                ScheduleListOptions::default()
                            } else {
                                lua.from_value(input)?
                            };
                            SchedulerRequest::List(options)
                        }
                        _ => {
                            if !matches!(input, Value::String(_)) {
                                return Err(mlua::Error::runtime(
                                    "scheduled task id must be a string",
                                ));
                            }
                            let id = String::from_lua(input, &lua)?;
                            match action {
                                "get" => SchedulerRequest::Get(id),
                                "cancel" => SchedulerRequest::Cancel(id),
                                _ => SchedulerRequest::Resume(id),
                            }
                        }
                    };
                    request
                        .authorize(&capabilities, context.allow_writes)
                        .map_err(error)?;
                    // 2. 【插件调度】【操作预算】实际宿主错误也计数，已提交任务不会随当前回调结束撤销
                    budget::checkpoint(&lua)?;
                    charge(&control, limits.system_calls)?;
                    let response = host
                        .scheduler(request.clone(), &context, &capabilities)
                        .await
                        .map_err(error)?;
                    budget::checkpoint(&lua)?;
                    request.validate_response(&response).map_err(error)?;
                    // 3. 【插件调度】【结果输出】检查宿主结果形状，再约束序列化输出大小
                    match response {
                        SchedulerResponse::Task(task) => {
                            result_value(&lua, &task, limits.output_bytes)
                        }
                        SchedulerResponse::Tasks(tasks) => {
                            result_value(&lua, &tasks, limits.output_bytes)
                        }
                        SchedulerResponse::Changed(changed) => Ok(Value::Boolean(changed)),
                    }
                }
            })?,
        )?;
    }
    api.set("scheduler", scheduler)
}
