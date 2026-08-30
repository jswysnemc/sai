//! TUI 侧的跨进程会话链接（P4）。
//!
//! 同一个会话被两个 sai 进程打开时只有一个能驱动轮次，另一个跟随：
//! 这是「单会话实例」的核心约束，两边都跑会各自往同一份会话状态里追加
//! 轮次，互相覆盖。
//!
//! - 本进程抢到租约 → [`LinkRole::Holder`]：行为与以前完全一致；
//! - 本进程是观察者 → [`LinkRole::Observer`]：跟随模式，见 [`follow`]；
//! - 拿不到 IPC 端点 → [`LinkRole::Detached`]：与 P3 完全一致的单进程行为。
//!
//! 会话切换（`/new`、`/resume`）后必须重新链接：旧链接持有的是旧会话的租约，
//! 继续用它发布事件会写进上一个会话的事件文件。

use crate::cli::repl_runtime::ReplRuntime;
use crate::i18n::text as t;
use crate::ipc::link::{LinkRole, SessionLink};
use crate::paths::SaiPaths;
use crate::runner::{session_holder, SessionOwner};
use crate::state::StateStore;
use crate::web::runs::WebEvent;
use std::path::Path;

/// TUI 与当前会话的跨进程链接。
pub(super) struct ReplSessionLink {
    /// 当前会话的链接；拿不到会话状态目录时为空
    link: Option<SessionLink>,
    /// 链接建立时的会话标识；与当前会话不一致就重建
    attached_session: String,
    /// 最近一次告知过用户的角色，避免重复打印提示
    announced: Option<LinkRole>,
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
    pub(super) async fn attach(paths: &SaiPaths, state: &StateStore) -> Self {
        let mut link = Self {
            link: None,
            attached_session: String::new(),
            announced: None,
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
            self.attached_session = session_id;
            return;
        };
        let cwd = crate::runtime_cwd::current_dir().unwrap_or_default();
        let workspace_id = crate::state::workspace_id_for_path(&cwd);
        let journal_path =
            crate::web::runs::session_event_path(&paths.state_dir, &workspace_id, &session_id);
        let (link, _bus) = SessionLink::attach(
            SessionOwner::Repl,
            &state_dir,
            &session_id,
            journal_path,
            &workspace_id,
        )
        .await;
        self.link = Some(link);
        self.attached_session = session_id;
    }
}

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
        "This session is held by another sai instance; this terminal follows it and will not run turns. Close that instance and this terminal takes over automatically.",
        "本会话正由另一个 sai 实例持有：本终端进入跟随模式，不会执行轮次；关闭那个实例后本终端会自动接管。",
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

/// 判断一次轮次提交是否必须被拒绝。
///
/// 观察者模式下本进程不驱动轮次：两个进程同时往同一份会话状态里追加轮次
/// 会互相覆盖。拒绝必须给出可操作的理由，不能只是"不行"。
///
/// 参数:
/// - `role`: 当前角色
/// - `state_dir`: 会话状态目录，用于读出持有者类型
///
/// 返回:
/// - 需要拒绝时的提示文本
pub(super) fn observer_turn_refusal(role: LinkRole, state_dir: &Path) -> Option<String> {
    if role != LinkRole::Observer {
        return None;
    }
    let owner = holder_owner(state_dir).unwrap_or_else(|| "sai".to_string());
    let base = t(
        "This session is driven by another sai instance; this terminal only follows it. Run the turn there, or close that instance — this terminal takes over within seconds.",
        "本会话由另一个 sai 实例驱动，本终端仅跟随：请在那个实例里执行本轮，或关闭它——本终端会在数秒内自动接管。",
    );
    Some(format!("{base}（{owner}）"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 持有者与单进程模式都不拒绝提交：单进程场景零回归。
    #[test]
    fn only_observers_refuse_turns() {
        let dir = tempfile::tempdir().unwrap();
        assert!(observer_turn_refusal(LinkRole::Holder, dir.path()).is_none());
        assert!(observer_turn_refusal(LinkRole::Detached, dir.path()).is_none());
    }

    /// 观察者模式给出可操作的拒绝理由，并点名持有者。
    #[test]
    fn observer_refusal_names_the_holder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("session-holder.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": 1,
                "session_id": "session",
                "owner": "web",
                "pid": 1_u32,
                "started_at": "2026-01-01T00:00:00Z",
                "heartbeat_at": "2026-01-01T00:00:00Z",
                "transport": null,
                "watchers": 0,
            }))
            .unwrap(),
        )
        .unwrap();
        let refusal = observer_turn_refusal(LinkRole::Observer, dir.path())
            .expect("观察者模式必须拒绝提交");
        assert!(refusal.contains("web"), "拒绝理由要点名持有者：{refusal}");
    }
}
