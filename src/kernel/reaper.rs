use crate::model::{
    ActorId, ActorKind, DecisionOutcome, EvidenceBundle, EvidenceInline, LeaseEffect, LeaseId,
    PendingWakeEffect, RecordId, TransitionDecision, TransitionKind, TransitionRecord, WorkPatch,
    WorkSnapshot, WorkStatus,
};

pub(crate) fn timeout_requeue_transition(
    snapshot: &WorkSnapshot,
    run_id: &str,
    lease_id: &LeaseId,
    reaped_at: crate::model::Timestamp,
) -> (TransitionDecision, TransitionRecord) {
    let next_snapshot = WorkSnapshot {
        status: WorkStatus::Todo,
        assignee_agent_id: None,
        active_lease_id: None,
        rev: snapshot.rev + 1,
        updated_at: reaped_at,
        ..snapshot.clone()
    };
    let summary = format!(
        "{:?} {:?} with next status {:?}",
        TransitionKind::TimeoutRequeue,
        DecisionOutcome::Accepted,
        WorkStatus::Todo
    );
    let decision = TransitionDecision {
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        next_snapshot: Some(next_snapshot),
        lease_effect: LeaseEffect::Release,
        pending_wake_effect: PendingWakeEffect::Retain,
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        summary: summary.clone(),
    };
    let record = TransitionRecord {
        record_id: RecordId::from(format!("record-{run_id}-timeout")),
        company_id: snapshot.company_id.clone(),
        work_id: snapshot.work_id.clone(),
        actor_kind: ActorKind::System,
        actor_id: ActorId::from("system"),
        run_id: Some(crate::model::RunId::from(run_id)),
        session_id: None,
        lease_id: Some(lease_id.clone()),
        expected_rev: snapshot.rev,
        contract_set_id: snapshot.contract_set_id.clone(),
        contract_rev: snapshot.contract_rev,
        before_status: snapshot.status,
        after_status: Some(WorkStatus::Todo),
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::TimeoutRequeue,
        patch: WorkPatch {
            summary: format!("timed out run {run_id}"),
            resolved_obligations: Vec::new(),
            declared_risks: Vec::new(),
        },
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        evidence_inline: Some(EvidenceInline { summary }),
        evidence_refs: Vec::new(),
        happened_at: reaped_at,
    };

    (decision, record)
}
