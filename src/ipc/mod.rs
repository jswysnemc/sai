//! 本地 IPC：让共享同一个会话 `state_dir` 的两个 sai 进程（TUI 进程 `sai` 与 Web 进程
//! `sai web`）互相通信。
//!
//! 这是「单会话实例、多前端同步」改造的第一阶段（P1），只交付：
//!
//! - [`frame`]：JSONL 帧编解码，带单行长度上限
//! - [`transport`]：跨平台双向帧传输（Unix domain socket / Windows named pipe）、
//!   端点推导、持有者探活
//!
//! 业务语义已随 P4 落地：[`link`] 负责持有者选举、IPC 监听、观察者接入与
//! 心跳超时接管；事件订阅与断点补发复用 `EventJournal` 与 SessionActor。
//! 因此本模块目前没有外部调用方，大量 API 带 `#[allow(dead_code)]` 并在注释里标注了
//! 后续阶段会由谁调用——这是预期状态，不要把这些 allow 当成遗留垃圾删掉。

pub(crate) mod frame;
pub(crate) mod link;
pub(crate) mod transport;

#[allow(unused_imports)]
pub(crate) use frame::{read_frame, write_frame, Frame};
#[allow(unused_imports)]
pub(crate) use link::{LinkRole, SessionLink};
#[allow(unused_imports)]
pub(crate) use transport::{
    probe_holder, transport_for_state_dir, Endpoint, SessionStream, SessionTransport,
};
