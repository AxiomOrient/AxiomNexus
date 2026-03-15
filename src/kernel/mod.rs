#![allow(dead_code)]

mod apply;
mod claim;
mod decide;
mod reaper;
mod record;
mod replay;
mod session;
mod wake;

use std::time::SystemTime;

pub(crate) use self::replay::ReplayError;
pub(crate) use self::wake::WakeRunPlan;

use crate::model::{
    ContractSet, EvidenceBundle, PendingWake, ReasonCode, SessionInvalidationReason, TaskSession,
    TransitionDecision, TransitionIntent, WorkId, WorkLease, WorkSnapshot, WorkStatus,
};

pub(crate) fn decide_transition(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    pending_wake: Option<&PendingWake>,
    contract: &ContractSet,
    evidence: &EvidenceBundle,
    intent: &TransitionIntent,
) -> TransitionDecision {
    decide::decide_transition(snapshot, lease, pending_wake, contract, evidence, intent)
}

pub(crate) fn command_gate_specs(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    contract: &ContractSet,
    intent: &TransitionIntent,
) -> Vec<crate::model::GateSpec> {
    decide::command_gate_specs(snapshot, lease, contract, intent)
}

pub(crate) fn claim_lease(snapshot: &WorkSnapshot) -> Result<(), ReasonCode> {
    claim::claim_lease(snapshot)
}

pub(crate) fn claim_transition_record(
    snapshot: &WorkSnapshot,
    lease: &WorkLease,
    happened_at: SystemTime,
) -> crate::model::TransitionRecord {
    claim::claim_transition_record(snapshot, lease, happened_at)
}

pub(crate) fn transition_record(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    intent: &TransitionIntent,
    decision: &TransitionDecision,
    session_id: Option<&crate::model::SessionId>,
    happened_at: SystemTime,
) -> crate::model::TransitionRecord {
    record::transition_record(snapshot, lease, intent, decision, session_id, happened_at)
}

pub(crate) fn apply_snapshot_patch(
    snapshot: &WorkSnapshot,
    intent: &TransitionIntent,
    next_status: WorkStatus,
    lease_effect: crate::model::LeaseEffect,
) -> WorkSnapshot {
    apply::apply_snapshot_patch(snapshot, intent, next_status, lease_effect)
}

pub(crate) fn merge_wake(
    existing: Option<&PendingWake>,
    incoming_reason: &str,
    incoming_obligations: &[String],
    merged_at: SystemTime,
    work_id: WorkId,
) -> PendingWake {
    wake::merge_wake(
        existing,
        incoming_reason,
        incoming_obligations,
        merged_at,
        work_id,
    )
}

pub(crate) fn wake_run_plan(
    has_open_lease: bool,
    has_runnable_run: bool,
    has_runnable_agent: bool,
) -> WakeRunPlan {
    wake::wake_run_plan(has_open_lease, has_runnable_run, has_runnable_agent)
}

pub(crate) fn advance_session(
    existing: Option<&TaskSession>,
    candidate: TaskSession,
    invalidation_reason: Option<SessionInvalidationReason>,
) -> TaskSession {
    session::advance_session(existing, candidate, invalidation_reason)
}

pub(crate) fn next_session_from_decision(
    existing: Option<TaskSession>,
    contract_rev: u32,
    record: &crate::model::TransitionRecord,
    decision: &TransitionDecision,
) -> Option<TaskSession> {
    record::next_session_from_decision(existing, contract_rev, record, decision)
}

pub(crate) fn session_invalidation_reason(
    existing: &TaskSession,
    agent_id: &crate::model::AgentId,
    work_id: &WorkId,
    cwd: &str,
    runtime: crate::model::RuntimeKind,
) -> Option<SessionInvalidationReason> {
    session::session_invalidation_reason(existing, agent_id, work_id, cwd, runtime)
}

pub(crate) fn replay_snapshot_from_records(
    base: &WorkSnapshot,
    records: &[crate::model::TransitionRecord],
) -> Result<WorkSnapshot, replay::ReplayError> {
    replay::replay_snapshot_from_records(base, records)
}

pub(crate) fn replay_base_snapshot(snapshot: &WorkSnapshot) -> WorkSnapshot {
    replay::replay_base_snapshot(snapshot)
}

pub(crate) fn replay_snapshot_mismatch(
    record_id: Option<crate::model::RecordId>,
    message: &str,
) -> ReplayError {
    replay::replay_snapshot_mismatch(record_id, message)
}

pub(crate) fn timeout_requeue_transition(
    snapshot: &WorkSnapshot,
    run_id: &str,
    lease_id: &crate::model::LeaseId,
    reaped_at: crate::model::Timestamp,
) -> (
    crate::model::TransitionDecision,
    crate::model::TransitionRecord,
) {
    reaper::timeout_requeue_transition(snapshot, run_id, lease_id, reaped_at)
}

#[cfg(test)]
mod tests;
