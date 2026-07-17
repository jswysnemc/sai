pub(crate) mod schema;

mod model;
mod policy;
mod process_owner;
mod remote_control;
mod replay;
mod repository;
mod sequence;
mod summary;
mod terminator;
mod transport;
mod transport_audit;
mod transport_event;
mod transport_model;
mod transport_replay;

pub(crate) use model::{
    NewRuntimeProcessEventInput, NewRuntimeProcessRecord, NewRuntimeRecoveryRecord, OwnerKind,
    ProcessKind, RuntimeProcessStatus, RuntimeRecoveryKind, RuntimeRecoveryStatus,
};
#[allow(unused_imports)]
pub(crate) use policy::{
    apply_command_mode_exit_policy, apply_connection_close_policy,
    apply_connection_close_policy_with, ConnectionClosePolicyOutcome,
};
#[cfg(test)]
pub(crate) use process_owner::audit_dead_process_owners_with;
pub(crate) use process_owner::{audit_dead_process_owners, audit_stale_subagent_owners};
#[allow(unused_imports)]
pub(crate) use remote_control::{
    advance_remote_control_cursor, load_remote_control_state, record_remote_control_auth_failure,
    upsert_remote_control_state, RemoteControlDesiredState, RemoteControlState,
    RemoteControlStateUpsert,
};
pub(crate) use repository::{
    append_next_process_event, record_process, record_recovery, session_summary,
};
pub(crate) use sequence::audit_sequence_gaps;
#[allow(unused_imports)]
pub(crate) use summary::{
    has_visible_runtime_recovery, RuntimeRecoveryFailureSummary, RuntimeRecoverySummary,
};
#[allow(unused_imports)]
pub(crate) use terminator::ProcessTerminator;
#[allow(unused_imports)]
pub(crate) use transport::{
    advance_gateway_transport_cursor, load_gateway_transport_state, record_gateway_transport_close,
};
pub(crate) use transport_audit::audit_gateway_transport_replay;
#[allow(unused_imports)]
pub(crate) use transport_event::{load_gateway_transport_events, record_gateway_transport_event};
#[allow(unused_imports)]
pub(crate) use transport_model::{
    RuntimeTransportEvent, RuntimeTransportKind, RuntimeTransportReplayDecision,
    RuntimeTransportState, RuntimeTransportStateUpsert,
};
pub(crate) use transport_replay::begin_gateway_transport_replay_event;
