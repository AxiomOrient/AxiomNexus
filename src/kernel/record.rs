use crate::model::{
    ActorId, ActorKind, DecisionOutcome, EvidenceInline, RecordId, SessionId, TaskSession,
    TransitionDecision, TransitionIntent, TransitionKind, TransitionRecord, WorkLease,
    WorkSnapshot,
};

pub(crate) fn transition_record(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    intent: &TransitionIntent,
    decision: &TransitionDecision,
    session_id: Option<&SessionId>,
    happened_at: crate::model::Timestamp,
) -> TransitionRecord {
    TransitionRecord {
        record_id: RecordId::from(format!(
            "record-{}-{}-{:?}",
            snapshot.work_id,
            snapshot.rev + 1,
            intent.kind
        )),
        company_id: snapshot.company_id.clone(),
        work_id: intent.work_id.clone(),
        actor_kind: actor_kind_for_intent(intent.kind),
        actor_id: actor_id_for_intent(intent),
        run_id: lease.and_then(|current_lease| current_lease.run_id.clone()),
        session_id: session_id.cloned(),
        lease_id: lease.map(|current_lease| current_lease.lease_id.clone()),
        expected_rev: intent.expected_rev,
        contract_set_id: snapshot.contract_set_id.clone(),
        contract_rev: snapshot.contract_rev,
        before_status: snapshot.status,
        after_status: decision.next_snapshot.as_ref().map(|next| next.status),
        outcome: decision.outcome,
        reasons: decision.reasons.clone(),
        kind: intent.kind,
        patch: intent.patch.clone(),
        gate_results: decision.gate_results.clone(),
        evidence: decision.evidence.clone(),
        evidence_inline: Some(EvidenceInline {
            summary: decision.summary.clone(),
        }),
        evidence_refs: decision.evidence.artifact_refs.clone(),
        happened_at,
    }
}

pub(crate) fn actor_kind_for_intent(kind: TransitionKind) -> ActorKind {
    match kind {
        TransitionKind::Claim
        | TransitionKind::ProposeProgress
        | TransitionKind::Complete
        | TransitionKind::Block => ActorKind::Agent,
        TransitionKind::TimeoutRequeue => ActorKind::System,
        TransitionKind::Queue
        | TransitionKind::Reopen
        | TransitionKind::Cancel
        | TransitionKind::OverrideComplete => ActorKind::Board,
    }
}

fn actor_id_for_intent(intent: &TransitionIntent) -> ActorId {
    match actor_kind_for_intent(intent.kind) {
        ActorKind::Agent => ActorId::from(intent.agent_id.as_str()),
        ActorKind::Board => ActorId::from("board"),
        ActorKind::System => ActorId::from("system"),
    }
}

pub(crate) fn next_session_from_decision(
    existing: Option<TaskSession>,
    contract_rev: u32,
    record: &TransitionRecord,
    decision: &TransitionDecision,
) -> Option<TaskSession> {
    let existing = existing?;

    let mut candidate = existing.clone();
    candidate.contract_rev = contract_rev;
    candidate.last_record_id = Some(record.record_id.clone());
    candidate.updated_at = record.happened_at;

    match decision.outcome {
        DecisionOutcome::Accepted | DecisionOutcome::OverrideAccepted => {
            candidate.last_decision_summary = Some(decision.summary.clone());
            candidate.last_gate_summary = None;
        }
        DecisionOutcome::Rejected | DecisionOutcome::Conflict => {
            candidate.last_gate_summary = Some(decision.summary.clone());
        }
    }

    Some(crate::kernel::advance_session(
        Some(&existing),
        candidate,
        None,
    ))
}
