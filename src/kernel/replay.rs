use crate::model::{
    ActorKind, DecisionOutcome, RecordId, TransitionKind, TransitionRecord, WorkSnapshot,
    WorkStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplayFailureCode {
    WorkIdMismatch,
    ContractPinMismatch,
    RevGap,
    StatusMismatch,
    SnapshotMismatch,
    MissingAfterStatus,
    ActorKindMismatch,
}

impl ReplayFailureCode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::WorkIdMismatch => "work_id_mismatch",
            Self::ContractPinMismatch => "contract_pin_mismatch",
            Self::RevGap => "rev_gap",
            Self::StatusMismatch => "status_mismatch",
            Self::SnapshotMismatch => "snapshot_mismatch",
            Self::MissingAfterStatus => "missing_after_status",
            Self::ActorKindMismatch => "actor_kind_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayError {
    pub(crate) code: ReplayFailureCode,
    pub(crate) record_id: Option<RecordId>,
    pub(crate) message: String,
}

pub(crate) fn replay_snapshot_from_records(
    base: &WorkSnapshot,
    records: &[TransitionRecord],
) -> Result<WorkSnapshot, ReplayError> {
    let mut snapshot = base.clone();

    for record in records {
        if record.work_id != snapshot.work_id {
            return Err(replay_error(
                ReplayFailureCode::WorkIdMismatch,
                Some(record.record_id.clone()),
                "record work_id does not match snapshot",
            ));
        }
        if record.company_id != snapshot.company_id
            || record.contract_set_id != snapshot.contract_set_id
            || record.contract_rev != snapshot.contract_rev
        {
            return Err(replay_error(
                ReplayFailureCode::ContractPinMismatch,
                Some(record.record_id.clone()),
                "record contract pin does not match snapshot contract pin",
            ));
        }
        if record.expected_rev != snapshot.rev {
            return Err(replay_error(
                ReplayFailureCode::RevGap,
                Some(record.record_id.clone()),
                "record expected_rev does not match snapshot rev",
            ));
        }
        if record.before_status != snapshot.status {
            return Err(replay_error(
                ReplayFailureCode::StatusMismatch,
                Some(record.record_id.clone()),
                "record before_status does not match snapshot status",
            ));
        }

        if !matches!(
            record.outcome,
            DecisionOutcome::Accepted | DecisionOutcome::OverrideAccepted
        ) {
            continue;
        }

        let Some(after_status) = record.after_status else {
            return Err(replay_error(
                ReplayFailureCode::MissingAfterStatus,
                Some(record.record_id.clone()),
                "accepted record is missing after_status",
            ));
        };

        snapshot.status = after_status;
        snapshot.rev = record.expected_rev + 1;
        snapshot.updated_at = record.happened_at;

        match record.kind {
            TransitionKind::Claim | TransitionKind::ProposeProgress => {
                snapshot.active_lease_id = record.lease_id.clone();
                snapshot.assignee_agent_id = match record.actor_kind {
                    ActorKind::Agent => Some(record.actor_id.as_str().into()),
                    _ => {
                        return Err(replay_error(
                            ReplayFailureCode::ActorKindMismatch,
                            Some(record.record_id.clone()),
                            "claim/propose_progress replay requires agent actor",
                        ))
                    }
                };
            }
            TransitionKind::Complete | TransitionKind::Block => {
                snapshot.active_lease_id = None;
            }
            TransitionKind::Queue
            | TransitionKind::Reopen
            | TransitionKind::Cancel
            | TransitionKind::OverrideComplete
            | TransitionKind::TimeoutRequeue => {
                snapshot.active_lease_id = None;
                snapshot.assignee_agent_id = None;
            }
        }
    }

    Ok(snapshot)
}

pub(crate) fn replay_base_snapshot(snapshot: &WorkSnapshot) -> WorkSnapshot {
    WorkSnapshot {
        work_id: snapshot.work_id.clone(),
        company_id: snapshot.company_id.clone(),
        parent_id: snapshot.parent_id.clone(),
        kind: snapshot.kind,
        title: snapshot.title.clone(),
        body: snapshot.body.clone(),
        status: WorkStatus::Backlog,
        priority: snapshot.priority,
        assignee_agent_id: None,
        active_lease_id: None,
        rev: 0,
        contract_set_id: snapshot.contract_set_id.clone(),
        contract_rev: snapshot.contract_rev,
        created_at: snapshot.created_at,
        updated_at: snapshot.created_at,
    }
}

pub(crate) fn replay_snapshot_mismatch(record_id: Option<RecordId>, message: &str) -> ReplayError {
    replay_error(ReplayFailureCode::SnapshotMismatch, record_id, message)
}

fn replay_error(
    code: ReplayFailureCode,
    record_id: Option<RecordId>,
    message: &str,
) -> ReplayError {
    ReplayError {
        code,
        record_id,
        message: message.to_owned(),
    }
}
