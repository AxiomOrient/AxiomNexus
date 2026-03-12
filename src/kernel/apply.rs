use std::time::Duration;

use crate::model::{LeaseEffect, TransitionIntent, TransitionKind, WorkSnapshot, WorkStatus};

pub(crate) fn apply_snapshot_patch(
    snapshot: &WorkSnapshot,
    intent: &TransitionIntent,
    next_status: WorkStatus,
    lease_effect: LeaseEffect,
) -> WorkSnapshot {
    let mut next = snapshot.clone();
    next.status = next_status;
    next.rev = snapshot.rev + 1;
    next.updated_at = snapshot.updated_at + Duration::from_secs(1);

    match lease_effect {
        LeaseEffect::Acquire | LeaseEffect::Keep | LeaseEffect::Renew => {
            next.active_lease_id = Some(intent.lease_id.clone());
            next.assignee_agent_id = Some(intent.agent_id.clone());
        }
        LeaseEffect::Release => {
            next.active_lease_id = None;
        }
        LeaseEffect::None => {}
    }

    if matches!(
        intent.kind,
        TransitionKind::Queue
            | TransitionKind::Reopen
            | TransitionKind::Cancel
            | TransitionKind::OverrideComplete
            | TransitionKind::TimeoutRequeue
    ) {
        next.assignee_agent_id = None;
    }

    next
}
