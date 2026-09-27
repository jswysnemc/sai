//! 内置 TypeSafe Jev 功能：工具与 skills 暴露决策、权限自动审核与连接测试。
//!
//! 本模块只负责问题构造、HTTP 请求与答案校验；与 Agent 可见性状态的对接位于
//! `agent::jev_routing`，与权限代理的对接位于 `permission::jev_audit`。

pub(crate) mod audit;
mod candidates;
mod choice;
mod client;
pub(crate) mod probe;
mod selection;

pub(crate) use candidates::{pending_candidates, Candidate, CandidateKind};
pub(crate) use client::JevClient;
pub(crate) use selection::{
    build_questions, build_state, question_id, select, Selection, SelectionLimits,
};

#[cfg(test)]
mod live_tests;
