use crate::{
    kernel,
    model::{
        DecisionOutcome, EvidenceInline, GateResult, LeaseId, Timestamp, TransitionRecord,
        WorkPatch, WorkSnapshot,
    },
    port::store::{
        ActivityEntryView, BoardGateFailureView, BoardTransitionView, CommitDecisionReq,
    },
};

pub(crate) fn transition_summary(
    evidence_inline: Option<&EvidenceInline>,
    patch: &WorkPatch,
) -> String {
    evidence_inline
        .map(|evidence| evidence.summary.clone())
        .unwrap_or_else(|| patch.summary.clone())
}

pub(crate) fn failed_gate_details(gate_results: &[GateResult]) -> Vec<String> {
    gate_results
        .iter()
        .filter(|result| !result.passed)
        .map(|result| result.detail.clone())
        .collect()
}

pub(crate) fn is_gate_failure(outcome: DecisionOutcome, gate_results: &[GateResult]) -> bool {
    matches!(outcome, DecisionOutcome::Rejected) || gate_results.iter().any(|result| !result.passed)
}

pub(crate) fn transition_activity_entry_view(record: &TransitionRecord) -> ActivityEntryView {
    ActivityEntryView {
        event_kind: "transition".to_owned(),
        work_id: record.work_id.as_str().to_owned(),
        summary: transition_summary(record.evidence_inline.as_ref(), &record.patch),
        actor_kind: Some(record.actor_kind),
        actor_id: Some(record.actor_id.as_str().to_owned()),
        source: None,
        before_status: Some(record.before_status),
        after_status: record.after_status,
        outcome: Some(
            match record.outcome {
                DecisionOutcome::Accepted => "accepted",
                DecisionOutcome::Rejected => "rejected",
                DecisionOutcome::Conflict => "conflict",
                DecisionOutcome::OverrideAccepted => "override_accepted",
            }
            .to_owned(),
        ),
        evidence_summary: record
            .evidence_inline
            .as_ref()
            .map(|evidence| evidence.summary.clone()),
    }
}

pub(crate) fn board_transition_detail(
    record_id: impl Into<String>,
    work_id: impl Into<String>,
    kind: impl Into<String>,
    outcome: impl Into<String>,
    summary: impl Into<String>,
) -> BoardTransitionView {
    BoardTransitionView {
        record_id: record_id.into(),
        work_id: work_id.into(),
        kind: kind.into(),
        outcome: outcome.into(),
        summary: summary.into(),
    }
}

pub(crate) fn board_gate_failure_detail(
    record_id: impl Into<String>,
    work_id: impl Into<String>,
    outcome: impl Into<String>,
    failed_gates: Vec<String>,
) -> BoardGateFailureView {
    BoardGateFailureView {
        record_id: record_id.into(),
        work_id: work_id.into(),
        outcome: outcome.into(),
        failed_gates,
    }
}

pub(crate) fn timeout_requeue_commit_req(
    snapshot: &WorkSnapshot,
    run_id: &str,
    lease_id: &LeaseId,
    reaped_at: crate::model::Timestamp,
) -> CommitDecisionReq {
    let (decision, record) =
        kernel::timeout_requeue_transition(snapshot, run_id, lease_id, reaped_at);

    CommitDecisionReq::new(decision, record, None)
}

pub(crate) fn with_authoritative_commit_timestamp(
    mut req: CommitDecisionReq,
    happened_at: Timestamp,
) -> CommitDecisionReq {
    req.context.record.happened_at = happened_at;
    if let Some(snapshot) = req.decision.next_snapshot.as_mut() {
        snapshot.updated_at = happened_at;
    }
    if let Some(session) = req.effects.session.as_mut() {
        session.updated_at = happened_at;
    }
    req
}
