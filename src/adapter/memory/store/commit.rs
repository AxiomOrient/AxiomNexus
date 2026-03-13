use super::*;

pub(super) struct CommitAuthoritativeState {
    pub(super) snapshot: WorkSnapshot,
    pub(super) live_lease: Option<WorkLease>,
}

pub(super) struct PreparedCommitDecision {
    pub(super) req: CommitDecisionReq,
    pub(super) activity_event: TimedActivityEntry,
}

pub(super) fn load_commit_authoritative_state(
    state: &MemoryState,
    req: &CommitDecisionReq,
) -> Result<CommitAuthoritativeState, StoreError> {
    ensure_commit_preconditions(state, req)?;
    let snapshot = state
        .snapshots
        .get(req.context.record.work_id.as_str())
        .cloned()
        .ok_or_else(|| not_found("commit_decision", &req.context.record.work_id))?;
    let live_lease = req
        .context
        .record
        .lease_id
        .as_ref()
        .and_then(|lease_id| state.leases.get(lease_id.as_str()))
        .cloned()
        .filter(|lease| lease.released_at.is_none());

    Ok(CommitAuthoritativeState {
        snapshot,
        live_lease,
    })
}

pub(super) fn prepare_commit_decision(
    next: &mut MemoryState,
    req: CommitDecisionReq,
    _authoritative: CommitAuthoritativeState,
) -> PreparedCommitDecision {
    let authoritative_at = next.next_timestamp();
    let req = store_support::with_authoritative_commit_timestamp(req, authoritative_at);
    let activity_event = activity_entry_from_transition(&req.context.record);

    PreparedCommitDecision {
        req,
        activity_event,
    }
}

pub(super) fn execute_commit_decision_apply(
    next: &mut MemoryState,
    prepared: PreparedCommitDecision,
) -> Result<CommitDecisionRes, StoreError> {
    let req = prepared.req;
    let activity_event = prepared.activity_event;
    next.transition_records.push(req.context.record.clone());
    next.activity_events.push(activity_event.clone());

    match req.effects.pending_wake {
        PendingWakeEffect::Clear => {
            next.pending_wakes
                .remove(req.context.record.work_id.as_str());
        }
        PendingWakeEffect::Retain | PendingWakeEffect::Merge | PendingWakeEffect::None => {}
    }

    match req.effects.lease {
        LeaseEffect::Acquire => {
            let lease_id = req.context.record.lease_id.as_ref().ok_or_else(|| {
                conflict("commit_decision acquire requires a lease_id on the record")
            })?;
            let agent_id = claim_actor_agent_id(&req.context.record, "commit_decision")?;
            let _ = acquire_claim_lease(
                next,
                &req.context.record.work_id,
                &agent_id,
                lease_id,
                req.context.record.happened_at,
                "commit_decision",
            )?;
        }
        LeaseEffect::Keep | LeaseEffect::Renew => {
            if let Some(lease_id) = req.context.record.lease_id.as_ref() {
                if let Some(lease) = next.leases.get_mut(lease_id.as_str()) {
                    lease.released_at = None;
                    lease.release_reason = None;
                }
            }
        }
        LeaseEffect::Release => {
            let released_at = req.context.record.happened_at;
            if let Some(lease_id) = req.context.record.lease_id.as_ref() {
                if let Some(lease) = next.leases.get_mut(lease_id.as_str()) {
                    if release_reason_for(req.context.record.kind) == LeaseReleaseReason::Expired {
                        lease.expires_at = Some(released_at);
                    }
                    lease.released_at = Some(released_at);
                    lease.release_reason = Some(release_reason_for(req.context.record.kind));
                }
            }
        }
        LeaseEffect::None => {}
    }

    if let Some(snapshot) = req.decision.next_snapshot.clone() {
        next.snapshots.insert(snapshot.work_id.0.clone(), snapshot);
    }

    if let Some(session) = req.effects.session.clone() {
        next.sessions.insert(
            session_key_parts(&session.agent_id, &session.work_id),
            session,
        );
    }

    Ok(load_commit_decision_result(next, &req, activity_event))
}

pub(super) fn apply_commit_decision_to_state(
    next: &mut MemoryState,
    req: CommitDecisionReq,
) -> Result<CommitDecisionRes, StoreError> {
    execute_commit_decision_apply(
        next,
        PreparedCommitDecision {
            activity_event: activity_entry_from_transition(&req.context.record),
            req,
        },
    )
}

pub(super) fn load_commit_decision_result(
    next: &MemoryState,
    req: &CommitDecisionReq,
    activity_event: TimedActivityEntry,
) -> CommitDecisionRes {
    let work_id = req.context.record.work_id.clone();
    let snapshot = next.snapshots.get(work_id.as_str()).cloned();
    let lease = snapshot
        .as_ref()
        .and_then(|item| item.active_lease_id.as_ref())
        .and_then(|lease_id| next.leases.get(lease_id.as_str()))
        .cloned()
        .filter(|item| item.released_at.is_none());
    let pending_wake = next.pending_wakes.get(work_id.as_str()).cloned();
    let session = req.effects.session.clone();

    CommitDecisionRes {
        snapshot,
        lease,
        pending_wake,
        session,
        activity_event: Some(activity_event.view),
    }
}
