use crate::{
    model::{
        ActorId, ActorKind, DecisionOutcome, EvidenceBundle, EvidenceInline, GateResult,
        LeaseEffect, LeaseId, PendingWakeEffect, RecordId, TransitionDecision, TransitionKind,
        TransitionRecord, WorkPatch, WorkSnapshot, WorkStatus,
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

    CommitDecisionReq {
        decision: TransitionDecision {
            outcome: DecisionOutcome::Accepted,
            reasons: Vec::new(),
            next_snapshot: Some(next_snapshot),
            lease_effect: LeaseEffect::Release,
            pending_wake_effect: PendingWakeEffect::Retain,
            gate_results: Vec::new(),
            evidence: EvidenceBundle::default(),
            summary: summary.clone(),
        },
        record: TransitionRecord {
            record_id: RecordId::from(format!("record-{run_id}-timeout")),
            company_id: snapshot.company_id.clone(),
            work_id: snapshot.work_id.clone(),
            actor_kind: ActorKind::System,
            actor_id: ActorId::from("system"),
            lease_id: Some(lease_id.clone()),
            expected_rev: snapshot.rev,
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
        },
        session: None,
    }
}
