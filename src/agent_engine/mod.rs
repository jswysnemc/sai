mod lazy;
mod selector;

use crate::agent::AgentEvent;
use crate::llm::ChatResult;
use anyhow::Result;

pub(crate) use selector::build_external_engine;

/// 一轮对话的输入。
///
/// 只携带外部内核真正需要的东西：它自己维护对话历史，
/// sai 的消息组装、上下文压缩与记忆注入都不适用。
#[derive(Debug, Clone)]
pub(crate) struct TurnRequest {
    /// 用户本轮输入
    pub(crate) input: String,
    /// 随本轮提交的图片（data URL）
    pub(crate) image_urls: Vec<String>,
    /// 会话工作目录，作为 ACP 会话的根
    pub(crate) cwd: std::path::PathBuf,
}

/// 外部内核向 sai 投递事件的发送端。
///
/// 刻意不把 UI 回调传进内核：回调借自调用方栈上的闭包，不保证跨线程可移动，
/// 而内核的 future 会随整个对话被 `tokio::spawn`（网关路径）。
/// 用通道解耦后，内核侧只持有一个 Send 的发送端，回调留在原地由调用方驱动。
pub(crate) type EventSender = tokio::sync::mpsc::UnboundedSender<AgentEvent>;

/// 执行对话轮次的外部内核。
///
/// 原生内核不实现本 trait：它与 `Agent` 的会话状态、工具注册表、记忆存储深度耦合，
/// 强行抽象只会把耦合搬进 trait 而不减少它。这里只描述「把一轮对话交出去执行」
/// 这一件事，`Agent` 在轮次入口处按配置分流。
/// 必须同时是 `Send + Sync`：`&Agent` 会被跨线程传递（网关的 tokio::spawn），
/// 内核作为 Agent 的字段，其共享引用也要能跨线程。
#[async_trait::async_trait]
pub(crate) trait ExternalTurnEngine: Send + Sync {
    /// 内核标识，用于界面展示与日志。
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// 执行一轮对话，过程中通过 `events` 流式产出事件。
    ///
    /// 参数:
    /// - `request`: 本轮输入
    /// - `events`: 事件发送端
    ///
    /// 返回:
    /// - 本轮结果
    async fn run_turn(&mut self, request: TurnRequest, events: EventSender)
        -> Result<ChatResult>;

    /// 结束会话并回收子进程。
    ///
    /// 子进程设了 kill_on_drop，正常退出路径已有兜底；
    /// 这里是显式的提前关闭入口，待接入会话生命周期。
    ///
    /// 返回:
    /// - 关闭结果；失败不应阻断调用方退出
    #[allow(dead_code)]
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
