//! 基于 TypeSafe Jev 的工具与 skills 暴露决策。
//!
//! 本模块只负责候选、问题、HTTP 请求与筛选；与 Agent 可见性状态的对接位于
//! `agent::jev_routing`。

mod candidates;
mod client;
mod selection;

pub(crate) use candidates::{pending_candidates, Candidate, CandidateKind};
pub(crate) use client::JevClient;
pub(crate) use selection::{
    build_questions, build_state, question_id, select, Selection, SelectionLimits,
};

#[cfg(test)]
mod live_tests;
