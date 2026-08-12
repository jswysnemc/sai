//! Agent 侧 SSH 服务器管理能力。
//!
//! 这是一个可通过 `load` 渐进加载的工具组，让模型能够安全地通过 SSH 管理远程服务器：
//! 列出主机、执行命令、传输文件。安全红线由三部分共同保证：
//!
//! - **秘密不落模型**：口令、密码等秘密经独立的 [`secret`] 通道在前端与后端之间流转，
//!   绝不进入工具参数、结果、错误或模型上下文（见 [`redact`] 脱敏）。
//! - **默认安全**：工具组不 `load` 就不出现在模型工具列表；首次连接主机需确认指纹；
//!   高危命令逐次确认（见 [`danger`]）。
//! - **权限联动**：写类工具声明为 `writes`，Plan 模式禁止、Audited 模式逐次审计，
//!   全部经既有权限代理与审计日志。
//!
//! 连接与认证复用 `crate::web::ssh` 的既有实现，避免重复造轮子。

mod danger;
mod redact;
mod secret;
mod session;
mod tools;
mod transfer;

pub(crate) use tools::register;

// 供 Web API 与 CLI/TUI 前端接线使用的交互征询接口。
pub(crate) use secret::{
    decode_progress_marker, decode_resolved_marker, is_pending, is_secret_marker,
    pending_ssh_secrets, submit_secret, InteractiveKind, SecretRequest, SecretResponse,
};
