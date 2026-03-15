use crate::model::{
    ActorId, ActorKind, AgentStatus, DecisionOutcome, EvidenceBundle, EvidenceInline, ReasonCode,
    RecordId, TransitionIntent, TransitionKind, TransitionRecord, WorkLease, WorkPatch,
    WorkSnapshot, WorkStatus,
};

pub(crate) fn claim_lease(snapshot: &WorkSnapshot) -> Result<(), ReasonCode> {
    if snapshot.active_lease_id.is_some() {
        Err(ReasonCode::LeaseConflict)
    } else {
        Ok(())
    }
}

pub(super) fn actor_kind_for_intent(kind: TransitionKind) -> ActorKind {
    super::record::actor_kind_for_intent(kind)
}

pub(super) fn lease_is_stale(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    intent: &TransitionIntent,
) -> bool {
    if !requires_live_lease(intent.kind) {
        return false;
    }

    match (snapshot.active_lease_id.as_ref(), lease) {
        (Some(active_lease_id), Some(current_lease)) => {
            active_lease_id != &intent.lease_id
                || &current_lease.lease_id != active_lease_id
                || current_lease.agent_id != intent.agent_id
                || current_lease.released_at.is_some()
        }
        _ => true,
    }
}

pub(crate) fn claim_transition_record(
    snapshot: &WorkSnapshot,
    lease: &WorkLease,
    happened_at: crate::model::Timestamp,
) -> TransitionRecord {
    TransitionRecord {
        record_id: RecordId::from(format!(
            "record-{}-{}-Claim",
            snapshot.work_id,
            snapshot.rev + 1
        )),
        company_id: snapshot.company_id.clone(),
        work_id: snapshot.work_id.clone(),
        actor_kind: ActorKind::Agent,
        actor_id: ActorId::from(lease.agent_id.as_str()),
        run_id: lease.run_id.clone(),
        session_id: None,
        lease_id: Some(lease.lease_id.clone()),
        expected_rev: snapshot.rev,
        contract_set_id: snapshot.contract_set_id.clone(),
        contract_rev: snapshot.contract_rev,
        before_status: snapshot.status,
        after_status: Some(WorkStatus::Doing),
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::Claim,
        patch: WorkPatch::default(),
        gate_results: Vec::new(),
        evidence: EvidenceBundle {
            observed_agent_status: Some(AgentStatus::Active),
            observed_agent_company_id: Some(snapshot.company_id.clone()),
            ..EvidenceBundle::default()
        },
        evidence_inline: Some(EvidenceInline {
            summary: "Claim Accepted with next status Doing".to_owned(),
        }),
        evidence_refs: Vec::new(),
        happened_at,
    }
}

fn requires_live_lease(kind: TransitionKind) -> bool {
    matches!(
        kind,
        TransitionKind::ProposeProgress | TransitionKind::Complete | TransitionKind::Block
    )
}
