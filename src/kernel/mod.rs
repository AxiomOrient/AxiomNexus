#![allow(dead_code)]

mod apply;
mod claim;
mod decide;
mod replay;
mod session;
mod wake;

use std::time::SystemTime;

pub(crate) use self::wake::WakeRunPlan;

use crate::model::{
    ContractSet, EvidenceBundle, PendingWake, ReasonCode, TaskSession, TransitionDecision,
    TransitionIntent, WorkId, WorkLease, WorkSnapshot, WorkStatus,
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
    invalid_session: bool,
) -> TaskSession {
    session::advance_session(existing, candidate, invalid_session)
}

pub(crate) fn replay_snapshot_from_records(
    base: &WorkSnapshot,
    records: &[crate::model::TransitionRecord],
) -> Result<WorkSnapshot, replay::ReplayError> {
    replay::replay_snapshot_from_records(base, records)
}

#[cfg(test)]
mod tests;
