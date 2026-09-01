//! TUI 侧的跨进程会话链接（P4）。
//!
//! 同一个会话被两个 sai 进程打开时只有一个能驱动轮次：这是「单会话实例」
//! 的核心约束，两边都跑会各自往同一份会话状态里追加轮次，互相覆盖。
//!
//! 但对用户必须是无感的，因此轮次的**发起方**和**执行方**分开：
//!
//! - 本进程抢到租约 → [`LinkRole::Holder`]：本终端提出的轮次就地执行，
//!   同时受理跟随端上行过来的提交（见 [`ReplSessionLink::serve_holder_requests`]）；
//! - 本进程是观察者 → [`LinkRole::Observer`]：本终端照常输入，提交上行给
//!   持有者执行，回显与流式输出经事件流回来（见 [`follow`]）；
//! - 拿不到 IPC 端点 → [`LinkRole::Detached`]：与 P3 完全一致的单进程行为。
//!
//! 会话切换（`/new`、`/resume`）后必须重新链接：旧链接持有的是旧会话的租约，
//! 继续用它发布事件会写进上一个会话的事件文件。

use crate::agent::AgentMode;
use crate::cli::repl_clipboard::ReplClipboardState;
use crate::cli::repl_runtime::{QueuedSubmission, ReplRuntime};
use crate::i18n::text as t;
use crate::ipc::link::{HolderRequest, LinkRole, SessionLink, SubmittedRun};
use crate::paths::SaiPaths;
use crate::runner::{session_holder, SessionOwner};
use crate::state::StateStore;
use crate::web::runs::{StartRunRequest, WebEvent};
use anyhow::{bail, Result};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// TUI 与当前会话的跨进程链接。
pub(super) struct ReplSessionLink {
    /// 当前会话的链接；拿不到会话状态目录时为空
    link: Option<SessionLink>,
    /// 本进程是持有者时的事件总线，用于把用户回显广播给跟随端
    bus: Option<crate::runner::ActorHandle>,
    /// 链接建立时的会话标识；与当前会话不一致就重建
    attached_session: String,
    /// 最近一次告知过用户的角色，避免重复打印提示
    announced: Option<LinkRole>,
    /// 用户提交队列的共享句柄：持有者侧把上行的一轮放进来等主循环取走
    submission_queue: Option<Arc<Mutex<VecDeque<QueuedSubmission>>>>,
}

impl ReplSessionLink {
    /// 为当前会话建立链接，但先不打扰用户。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    /// - `state`: 当前会话状态
    ///
    /// 返回:
    /// - 会话链接
    pub(super) async fn attach(
        paths: &SaiPaths,
        state: &StateStore,
        submission_queue: Arc<Mutex<VecDeque<QueuedSubmission>>>,
    ) -> Self {
        let mut link = Self {
            link: None,
            bus: None,
            attached_session: String::new(),
            announced: None,
            submission_queue: Some(submission_queue),
        };
        link.reattach(paths, state).await;
        link
    }

    /// 返回当前角色。
    ///
    /// 返回:
    /// - 未建立链接时为 [`LinkRole::Detached`]
    pub(super) fn role(&self) -> LinkRole {
        self.link
            .as_ref()
            .map_or(LinkRole::Detached, SessionLink::role)
    }

    /// 把本终端的一次提交上行给会话持有者代跑。
    ///
    /// 观察者不持有 Agent，自己跑会与持有者互相覆盖同一份会话状态；
    /// 上行失败一律返回 `Err`，调用方必须把原因呈现给用户并退回输入框——
    /// 用户输入绝不能静默消失。
    ///
    /// 参数:
    /// - `submission`: 本终端收集好的提交
    /// - `session_id`: 当前会话标识
    /// - `workspace_id`: 当前工作区标识
    ///
    /// 返回:
    /// - 无
    pub(super) async fn forward_turn(
        &self,
        submission: &super::repl_input::ReplInputSubmission,
        session_id: &str,
        workspace_id: &str,
    ) -> Result<()> {
        let Some(link) = self.link.as_ref() else {
            bail!(
                "{}",
                t(
                    "This input was not run: the session holder is unreachable. Retry in a moment, or run the turn in the terminal that holds the session.",
                    "这次输入没有执行：连不上会话持有者。稍后重试，或到持有该会话的终端里执行。"
                )
            );
        };
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let request = StartRunRequest {
            kind: crate::web::runs::RunKind::Conversation,
            session_id: session_id.to_string(),
            input: submission.chat_input.message.clone(),
            agent_id: None,
            image_url: submission.chat_input.image_url.clone(),
            image_urls: Vec::new(),
            mode: Some(submission.mode.key().to_string()),
            provider_id: None,
            model: None,
            thinking_level: None,
            insert_at: crate::web::runs::QueueInsertAt::Turn,
        };
        let ack = link
            .submit(SubmittedRun {
                submit_id: format!("sub_{}", uuid::Uuid::new_v4().simple()),
                run_id,
                workspace_id: workspace_id.to_string(),
                workspace_path: crate::runtime_cwd::current_dir()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
                request,
            })
            .await?;
        if !ack.accepted {
            bail!(
                "{}",
                ack.reason.unwrap_or_else(|| t(
                    "the session holder rejected this input",
                    "会话持有者拒绝了这次输入"
                )
                .to_string())
            );
        }
        Ok(())
    }

    /// 广播一条用户回显，让所有跟随端看得到本终端发出的消息。
    ///
    /// 持有者是会话事件的唯一写者，回显也必须经它落地，否则跟随端只看到
    /// 回答看不到提问。
    ///
    /// 参数:
    /// - `echo_text`: 已展开粘贴内容的回显正文
    /// - `image_urls`: 本轮附带的图片
    ///
    /// 返回:
    /// - 无
    pub(super) fn broadcast_user_message(&self, echo_text: &str, image_urls: Vec<String>) {
        let Some(bus) = self.bus.as_ref() else {
            return;
        };
        if self.role() == LinkRole::Observer {
            return;
        }
        let event = WebEvent::new(
            &format!("run_{}", uuid::Uuid::new_v4().simple()),
            "",
            &self.attached_session,
            USER_SUBMITTED_EVENT,
            serde_json::json!({
                "input": echo_text,
                "image_urls": image_urls,
            }),
        );
        let _ = bus.emit(event);
    }

    /// 受理跟随端上行的一轮，放进用户提交队列等主循环取走。
    ///
    /// 参数:
    /// - `queue`: 用户提交队列
    /// - `request`: 上行的一轮参数
    ///
    /// 返回:
    /// - 该轮在本进程的状态（`queued`）
    fn accept_remote_submission(
        queue: &Arc<Mutex<VecDeque<QueuedSubmission>>>,
        request: &StartRunRequest,
    ) -> Result<String> {
        let mode = AgentMode::parse(request.mode.as_deref()).unwrap_or(AgentMode::Yolo);
        let mut clipboard = ReplClipboardState::default();
        let mut text = request.input.clone();
        let mut cursor = text.chars().count();
        if let Some(data_url) = request
            .image_url
            .clone()
            .or_else(|| request.image_urls.first().cloned())
        {
            clipboard.insert_image_data_url(&mut text, &mut cursor, data_url);
        }
        let mut item = QueuedSubmission::new(mode, text, clipboard);
        // 轮次间隔：本终端正在跑一轮时，上行的一轮排到本轮结束后
        item.insert_at = crate::cli::repl_runtime::QueueInsertAt::Turn;
        queue
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push_back(item);
        Ok("queued".to_string())
    }

    /// 消费跟随端上行的请求队列。
    ///
    /// 只碰共享状态（提交队列），不碰 `ReplRuntime`：主循环正卡在读键上，
    /// 这里抢不到它的借用。
    ///
    /// 参数:
    /// - `queue`: 用户提交队列
    /// - `rx`: 请求接收端
    ///
    /// 返回:
    /// - 无
    fn serve_holder_requests(
        queue: Arc<Mutex<VecDeque<QueuedSubmission>>>,
        mut rx: mpsc::UnboundedReceiver<HolderRequest>,
    ) {
        tokio::spawn(async move {
            while let Some(request) = rx.recv().await {
                match request {
                    HolderRequest::Submit { request, reply } => {
                        let outcome = Self::accept_remote_submission(&queue, &request.request);
                        let _ = reply.send(outcome);
                    }
                    HolderRequest::Abort { reply, .. } => {
                        // TUI 的中断只有本终端的 Ctrl+C 一个入口，远端请求不在本进程
                        let _ = reply.send(Ok(false));
                    }
                }
            }
        });
    }

    /// 会话切换或角色变化后重新链接并同步界面。
    ///
    /// 角色会随看门狗变化：持有者进程退出后本进程接管。接管必须即时告知，
    /// 否则用户看到的是"输入被拒但没有任何解释"。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    /// - `state`: 当前会话状态
    /// - `runtime`: TUI 运行期，用于渲染角色变化提示与跟随流
    ///
    /// 返回:
    /// - 无
    pub(super) async fn refresh(
        &mut self,
        paths: &SaiPaths,
        state: &StateStore,
        runtime: &mut ReplRuntime,
    ) -> anyhow::Result<()> {
        if self.link.is_none() || self.attached_session != state.session_id() {
            self.reattach(paths, state).await;
        }
        let previous = self.announced;
        let role = self.role();
        if previous == Some(role) {
            return Ok(());
        }
        self.announced = Some(role);
        match role {
            LinkRole::Holder | LinkRole::Detached => {
                runtime.stop_following();
                if previous == Some(LinkRole::Observer) {
                    let _ = runtime.record_meta(takeover_message());
                }
            }
            LinkRole::Observer => {
                if let Some(link) = self.link.as_ref() {
                    let (events_tx, events_rx) = tokio::sync::mpsc::unbounded_channel::<WebEvent>();
                    // 先订阅再提示：提示之后到达的事件不能被丢掉
                    let mut subscription = link.subscribe();
                    tokio::spawn(async move {
                        while let Some(event) = subscription.events.recv().await {
                            if events_tx.send(event).is_err() {
                                return;
                            }
                        }
                    });
                    runtime.follow_remote_stream(events_rx);
                }
                let _ = runtime.record_meta(observer_message(state.state_dir()));
            }
        }
        Ok(())
    }

    /// 重建当前会话的链接。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    /// - `state`: 当前会话状态
    ///
    /// 返回:
    /// - 无
    async fn reattach(&mut self, paths: &SaiPaths, state: &StateStore) {
        let session_id = state.session_id().to_string();
        let Ok((_, state_dir)) = crate::state::locate_session_dirs(paths, &session_id) else {
            self.link = None;
            self.bus = None;
            self.attached_session = session_id;
            return;
        };
        // 会话事件文件按规范化后的工作区 ID 分目录，必须与写侧走同一条规范化
        let workspace_id = crate::state::current_workspace_id().unwrap_or_default();
        let journal_path =
            crate::web::runs::session_event_path(&paths.state_dir, &workspace_id, &session_id);
        let (link, bus) = SessionLink::attach(
            SessionOwner::Repl,
            &state_dir,
            &session_id,
            journal_path,
            &workspace_id,
        )
        .await;
        // 受理执行器与角色无关：接管后本进程可能从跟随者变持有者，
        // 执行器必须提前就位，否则跟随端上行的一轮会被当成「持有者不能驱动」
        if let Some(queue) = self.submission_queue.clone() {
            let (tx, rx) = mpsc::unbounded_channel();
            link.set_holder_sink(tx);
            Self::serve_holder_requests(queue, rx);
        }
        self.link = Some(link);
        self.bus = Some(bus);
        self.attached_session = session_id;
    }
}

/// 跟随端用来还原用户回显的事件类型。
pub(super) const USER_SUBMITTED_EVENT: &str = "user.submitted";

/// 观察者模式下的提示。
///
/// 参数:
/// - `state_dir`: 会话状态目录，用于读出持有者类型
///
/// 返回:
/// - 面向用户的提示文本
fn observer_message(state_dir: &Path) -> String {
    let owner = holder_owner(state_dir).unwrap_or_else(|| "sai".to_string());
    let base = t(
        "This session is driven by another sai instance; this terminal follows it and sends its input there. Close that instance and this terminal takes over automatically.",
        "本会话正由另一个 sai 实例驱动：本终端跟随它，输入会交给那边执行；关闭那个实例后本终端会自动接管。",
    );
    format!("{base}（{owner}）")
}

/// 接管会话后的提示。
///
/// 返回:
/// - 面向用户的提示文本
fn takeover_message() -> String {
    t(
        "The other sai instance is gone; this terminal now holds the session.",
        "另一个 sai 实例已退出，本终端已接管该会话。",
    )
    .to_string()
}

/// 读出登记表里的持有者类型。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 持有者类型；读不到登记表时为空
fn holder_owner(state_dir: &Path) -> Option<String> {
    session_holder(state_dir).map(|holder| holder.owner)
}

/// 返回底栏常驻的主从角色标记。
///
/// 只有跟随者需要标记：持有者与单进程是常态，挂个「持有中」只是噪音。
///
/// 参数:
/// - `role`: 当前角色
///
/// 返回:
/// - 跟随者时的标记文本，其余情况为空
pub(super) fn role_badge(role: LinkRole) -> Option<String> {
    match role {
        LinkRole::Observer => Some(t("follower", "跟随中").to_string()),
        LinkRole::Holder | LinkRole::Detached => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use std::collections::VecDeque;

    /// 跟随端才挂角色标记：持有者与单进程是常态。
    #[test]
    fn only_followers_wear_a_role_badge() {
        assert!(role_badge(LinkRole::Observer).is_some());
        assert!(role_badge(LinkRole::Holder).is_none());
        assert!(role_badge(LinkRole::Detached).is_none());
    }

    /// 拼一个带 IHDR 的最小 PNG data URL：尺寸是从 PNG 头里读出来的。
    fn png_data_url(width: u32, height: u32) -> String {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )
    }

    /// 上行的一轮必须带着图片进队列：丢掉附件会让占位块退化成字面文本。
    #[test]
    fn remote_submission_keeps_the_image_attachment() {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let data_url = png_data_url(800, 600);
        let request = crate::web::runs::StartRunRequest {
            kind: crate::web::runs::RunKind::Conversation,
            session_id: "session".to_string(),
            input: "看图".to_string(),
            agent_id: None,
            image_url: Some(data_url.clone()),
            image_urls: Vec::new(),
            mode: Some("plan".to_string()),
            provider_id: None,
            model: None,
            thinking_level: None,
            insert_at: crate::web::runs::QueueInsertAt::Turn,
        };
        ReplSessionLink::accept_remote_submission(&queue, &request).unwrap();

        let queued = queue.lock().unwrap();
        assert_eq!(queued.len(), 1);
        // 占位块带着真实尺寸，写死 0x0 会让回显里出现假尺寸
        assert!(queued[0].text.contains("[image 1 800x600]"));
        let chat = queued[0].clipboard.to_chat_input(&queued[0].text);
        assert_eq!(chat.image_url.as_deref(), Some(data_url.as_str()));
        assert!(chat.message.contains("看图"));
    }
}
