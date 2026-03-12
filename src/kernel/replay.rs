use crate::model::{ActorKind, DecisionOutcome, TransitionKind, TransitionRecord, WorkSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayError {
    pub(crate) message: String,
}

pub(crate) fn replay_snapshot_from_records(
    base: &WorkSnapshot,
    records: &[TransitionRecord],
) -> Result<WorkSnapshot, ReplayError> {
    let mut snapshot = base.clone();

    for record in records {
        if record.work_id != snapshot.work_id {
            return Err(replay_error("record work_id does not match snapshot"));
        }
        if record.expected_rev != snapshot.rev {
            return Err(replay_error(
                "record expected_rev does not match snapshot rev",
            ));
        }
        if record.before_status != snapshot.status {
            return Err(replay_error(
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
            return Err(replay_error("accepted record is missing after_status"));
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

fn replay_error(message: &str) -> ReplayError {
    ReplayError {
        message: message.to_owned(),
    }
}
