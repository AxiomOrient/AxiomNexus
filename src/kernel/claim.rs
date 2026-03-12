use crate::model::{
    ActorKind, ReasonCode, TransitionIntent, TransitionKind, WorkLease, WorkSnapshot,
};

pub(crate) fn claim_lease(snapshot: &WorkSnapshot) -> Result<(), ReasonCode> {
    if snapshot.active_lease_id.is_some() {
        Err(ReasonCode::LeaseConflict)
    } else {
        Ok(())
    }
}

pub(super) fn actor_kind_for_intent(kind: TransitionKind) -> ActorKind {
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

fn requires_live_lease(kind: TransitionKind) -> bool {
    matches!(
        kind,
        TransitionKind::ProposeProgress | TransitionKind::Complete | TransitionKind::Block
    )
}
