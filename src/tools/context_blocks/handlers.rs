use crate::state::context_blocks::CompressRequest;
use crate::state::StateStore;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StatusRequest {
    offset: usize,
    limit: Option<usize>,
    block_offset: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchRequest {
    query: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreRequest {
    message_id: String,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

/// 【上下文】【工具执行】解析工具参数并委托当前会话状态处理
/// 参数: state 为绑定会话，name 为工具名，args 为 JSON 参数；返回有界 JSON 结果
pub(super) fn execute(state: &StateStore, name: &str, mut args: Value) -> Result<String> {
    // 1. 【上下文】【内部参数】注册表注入的沙盒标记不属于工具公开参数
    if let Some(object) = args.as_object_mut() {
        object.remove("_sai_sandbox");
    }
    match name {
        "context_status" => {
            let args: StatusRequest = serde_json::from_value(args)?;
            Ok(serde_json::to_string(&state.context_block_catalog(
                args.offset,
                args.limit.unwrap_or(20),
                args.block_offset,
            )?)?)
        }
        "compress_context" => {
            let args: CompressRequest = serde_json::from_value(args)?;
            Ok(serde_json::to_string(
                &state.compress_context_block(&args)?,
            )?)
        }
        "search_context" => {
            let args: SearchRequest = serde_json::from_value(args)?;
            Ok(serde_json::to_string(&state.search_context_blocks(
                &args.query,
                args.limit.unwrap_or(10),
            )?)?)
        }
        "restore_context" => {
            let args: RestoreRequest = serde_json::from_value(args)?;
            Ok(serde_json::to_string(&state.restore_context_message(
                &args.message_id,
                args.offset,
                args.limit.unwrap_or(4000),
            )?)?)
        }
        _ => bail!("unknown context tool: {name}"),
    }
}
