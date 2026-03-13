use super::*;

pub(super) struct CommitAuthoritativeState {
    pub(super) snapshot: WorkDoc,
    pub(super) live_lease: Option<LeaseDoc>,
}

pub(super) struct PreparedCommitDecision {
    pub(super) req: CommitDecisionReq,
    pub(super) transition_doc: TransitionRecordDoc,
    pub(super) activity_doc: ActivityEventDoc,
    pub(super) run_doc: Option<RunDoc>,
    pub(super) run_activity_doc: Option<ActivityEventDoc>,
    pub(super) snapshot_doc: Option<WorkDoc>,
    pub(super) lease_doc: Option<LeaseDoc>,
    pub(super) session_doc: Option<SessionDoc>,
}

pub(super) fn load_commit_authoritative_state(
    store: &SurrealStore,
    req: &CommitDecisionReq,
) -> Result<CommitAuthoritativeState, StoreError> {
    let snapshot = store
        .select_record::<WorkDoc>(&work_record_id(req.context.record.work_id.as_str()))?
        .ok_or_else(|| not_found("commit_decision", req.context.record.work_id.as_str()))?;
    let live_lease = match req.context.record.lease_id.as_ref() {
        Some(lease_id) => store
            .select_record::<LeaseDoc>(&lease_record_id(lease_id.as_str()))?
            .filter(|lease| lease.released_at_secs.is_none()),
        None => None,
    };
    ensure_commit_preconditions(&snapshot, live_lease.as_ref(), req)?;

    Ok(CommitAuthoritativeState {
        snapshot,
        live_lease,
    })
}

pub(super) fn prepare_commit_decision(
    store: &SurrealStore,
    req: CommitDecisionReq,
    authoritative: CommitAuthoritativeState,
) -> Result<PreparedCommitDecision, StoreError> {
    let authoritative_at_secs = store.update_store_meta(|meta| {
        meta.tick += 1;
        meta.tick
    })?;
    let req =
        store_support::with_authoritative_commit_timestamp(req, timestamp(authoritative_at_secs));

    let transition_doc = TransitionRecordDoc::from_model(&req.context.record)?;
    let activity_doc = ActivityEventDoc::from_transition_record(&req.context.record);
    let claim_acquire = match req.effects.lease {
        LeaseEffect::Acquire => {
            let lease_id = req.context.record.lease_id.as_ref().ok_or_else(|| {
                conflict("commit_decision acquire requires a lease_id on the record")
            })?;
            let agent_id = claim_actor_agent_id(&req.context.record, "commit_decision")?;
            Some(prepare_claim_acquire(
                store,
                &authoritative.snapshot,
                &agent_id,
                lease_id,
                timestamp_secs(req.context.record.happened_at),
                "commit_decision",
            )?)
        }
        _ => None,
    };

    let lease_doc = match req.effects.lease {
        LeaseEffect::Acquire => claim_acquire
            .as_ref()
            .map(|prepared| LeaseDoc::from_model(&prepared.lease)),
        LeaseEffect::Keep | LeaseEffect::Renew => {
            authoritative.live_lease.clone().map(|lease| LeaseDoc {
                released_at_secs: None,
                release_reason: None,
                ..lease
            })
        }
        LeaseEffect::Release => authoritative.live_lease.clone().map(|lease| LeaseDoc {
            expires_at_secs: (release_reason_for(req.context.record.kind)
                == LeaseReleaseReason::Expired)
                .then(|| timestamp_secs(req.context.record.happened_at)),
            released_at_secs: Some(timestamp_secs(req.context.record.happened_at)),
            release_reason: Some(
                lease_release_reason_label(release_reason_for(req.context.record.kind)).to_owned(),
            ),
            ..lease
        }),
        LeaseEffect::None => None,
    };

    Ok(PreparedCommitDecision {
        snapshot_doc: req
            .decision
            .next_snapshot
            .as_ref()
            .map(WorkDoc::from_snapshot),
        session_doc: req.effects.session.as_ref().map(SessionDoc::from_model),
        run_doc: claim_acquire.as_ref().map(|prepared| prepared.run.clone()),
        run_activity_doc: claim_acquire
            .as_ref()
            .and_then(|prepared| prepared.run_activity.clone()),
        lease_doc,
        transition_doc,
        activity_doc,
        req,
    })
}

pub(super) fn execute_commit_decision_transaction(
    store: &SurrealStore,
    prepared: &PreparedCommitDecision,
) -> Result<(), StoreError> {
    store.runtime.block_on(async {
        let mut query = String::from("BEGIN TRANSACTION;\n");
        query.push_str(
            "UPSERT type::record('transition_record', $transition_id) CONTENT $transition_doc;\n",
        );
        query.push_str(
            "UPSERT type::record('activity_event', $activity_id) CONTENT $activity_doc;\n",
        );
        if prepared.run_doc.is_some() {
            query.push_str("UPSERT type::record('run', $run_id) CONTENT $run_doc;\n");
        }
        if prepared.run_activity_doc.is_some() {
            query.push_str(
                "UPSERT type::record('activity_event', $run_activity_id) CONTENT $run_activity_doc;\n",
            );
        }
        if prepared.snapshot_doc.is_some() {
            query.push_str("UPSERT type::record('work', $work_id) CONTENT $work_doc;\n");
        }
        if prepared.req.context.record.lease_id.is_some() && prepared.lease_doc.is_some() {
            query.push_str("UPSERT type::record('lease', $lease_id) CONTENT $lease_doc;\n");
        }
        if matches!(
            prepared.req.effects.pending_wake,
            crate::model::PendingWakeEffect::Clear
        ) {
            query.push_str("DELETE type::record('pending_wake', $pending_wake_id);\n");
        }
        if prepared.session_doc.is_some() {
            query.push_str(
                "UPSERT type::record('task_session', $session_key) CONTENT $session_doc;\n",
            );
        }
        query.push_str("COMMIT TRANSACTION;");

        let mut request = store
            .db
            .query(query)
            .bind(("transition_id", prepared.req.context.record.record_id.to_string()))
            .bind(("transition_doc", prepared.transition_doc.clone()))
            .bind(("activity_id", prepared.activity_doc.event_id.clone()))
            .bind(("activity_doc", prepared.activity_doc.clone()));

        if let Some(run_doc) = prepared.run_doc.clone() {
            let run_id = run_doc.run_id.clone();
            request = request.bind(("run_id", run_id)).bind(("run_doc", run_doc));
        }
        if let Some(run_activity_doc) = prepared.run_activity_doc.clone() {
            let run_activity_id = run_activity_doc.event_id.clone();
            request = request
                .bind(("run_activity_id", run_activity_id))
                .bind(("run_activity_doc", run_activity_doc));
        }
        if let Some(work_doc) = prepared.snapshot_doc.clone() {
            request = request
                .bind(("work_id", prepared.req.context.record.work_id.to_string()))
                .bind(("work_doc", work_doc));
        }
        if let Some(lease_doc) = prepared.lease_doc.clone() {
            if let Some(lease_id) = prepared.req.context.record.lease_id.as_ref() {
                request = request
                    .bind(("lease_id", lease_id.to_string()))
                    .bind(("lease_doc", lease_doc));
            }
        }
        if matches!(
            prepared.req.effects.pending_wake,
            crate::model::PendingWakeEffect::Clear
        ) {
            request = request.bind((
                "pending_wake_id",
                prepared.req.context.record.work_id.to_string(),
            ));
        }
        if let Some(session_doc) = prepared.session_doc.clone() {
            if let Some(session) = prepared.req.effects.session.as_ref() {
                request = request
                    .bind((
                        "session_key",
                        session_record_id_parts(session.agent_id.as_str(), session.work_id.as_str()),
                    ))
                    .bind(("session_doc", session_doc));
            }
        }

        request
            .await
            .map_err(|error| unavailable(&format!("surreal commit_decision failed: {error}")))?
            .check()
            .map_err(|error| unavailable(&format!("surreal commit_decision check failed: {error}")))?;
        Ok::<(), StoreError>(())
    })
}

pub(super) fn load_commit_decision_result(
    store: &SurrealStore,
    prepared: PreparedCommitDecision,
) -> Result<CommitDecisionRes, StoreError> {
    let snapshot = store
        .select_record::<WorkDoc>(&work_record_id(
            prepared.req.context.record.work_id.as_str(),
        ))?
        .map(WorkDoc::into_snapshot)
        .transpose()?;
    let lease = snapshot
        .as_ref()
        .and_then(|item| item.active_lease_id.as_ref())
        .and_then(|lease_id| {
            store
                .select_record::<LeaseDoc>(&lease_record_id(lease_id.as_str()))
                .ok()
        })
        .flatten()
        .filter(|item| item.released_at_secs.is_none())
        .map(LeaseDoc::into_model)
        .transpose()?;
    let pending_wake = store
        .select_record::<PendingWakeDoc>(&pending_wake_record_id(
            prepared.req.context.record.work_id.as_str(),
        ))?
        .map(PendingWakeDoc::into_model)
        .transpose()?;

    Ok(CommitDecisionRes {
        snapshot,
        lease,
        pending_wake,
        session: prepared.req.effects.session,
        activity_event: Some(prepared.activity_doc.into_view()?),
    })
}
