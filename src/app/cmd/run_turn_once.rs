use serde::Serialize;

use crate::{
    model::{
        AgentStatus, DecisionOutcome, EvidenceBundle, LeaseId, RunId, SessionInvalidationReason,
        TransitionKind,
    },
    port::runtime::RuntimePort,
    port::store::{
        ClaimLeaseReq, QueuedRunCandidate, RuntimeStorePort, RuntimeTurnContext, StoreError,
        StoreErrorKind,
    },
};

use super::{
    resume_session::{handle_resume_session, ResumeSessionCmd},
    submit_intent::{collect_runtime_observation_evidence, commit_runtime_intent, SubmitIntentAck},
    RUNTIME_RESUME_POLICY,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunTurnOnceReq {
    pub(crate) run_id: RunId,
    pub(crate) cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RunTurnOnceResp {
    pub(crate) runtime_policy: &'static str,
    pub(crate) run_id: String,
    pub(crate) work_id: String,
    pub(crate) agent_id: String,
    pub(crate) pending_wake_count: u32,
    pub(crate) resumed: bool,
    pub(crate) repair_count: u8,
    pub(crate) session_reset_reason: Option<SessionInvalidationReason>,
    pub(crate) runtime_session_id: String,
    pub(crate) intent_kind: TransitionKind,
    pub(crate) changed_file_count: usize,
    pub(crate) command_result_count: usize,
    pub(crate) observed_agent_status: Option<crate::model::AgentStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunTurnBase {
    work_id: String,
    agent_id: String,
    pending_wake_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommittedRunTurn {
    resume: super::resume_session::ResumeSessionAck,
    evidence: EvidenceBundle,
    commit: SubmitIntentAck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RunFinalization {
    KeepRunning,
    MarkCompleted,
    MarkFailed {
        reason: &'static str,
        error: StoreError,
    },
}

pub(crate) fn handle_run_turn_once(
    store: &(impl RuntimeStorePort + crate::port::store::CommandStorePort),
    runtime: &impl RuntimePort,
    req: RunTurnOnceReq,
) -> Result<RunTurnOnceResp, StoreError> {
    let turn = ensure_claimed_turn(store, &req.run_id)?;
    let turn_base = run_turn_base(&turn);
    let committed_turn = match resume_and_commit_turn(store, runtime, &req, &turn) {
        Ok(committed_turn) => committed_turn,
        Err(error) if error.kind == StoreErrorKind::Conflict => {
            store.mark_run_failed(&req.run_id, "decision_conflict")?;
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    apply_run_finalization(
        store,
        &req.run_id,
        run_finalization(
            committed_turn.resume.intent_kind,
            committed_turn.commit.outcome,
            &committed_turn.commit.summary,
        ),
    )?;

    Ok(run_turn_response(&req.run_id, turn_base, committed_turn))
}

fn ensure_claimed_turn(
    store: &impl RuntimeStorePort,
    run_id: &RunId,
) -> Result<RuntimeTurnContext, StoreError> {
    let initial_turn = store.load_runtime_turn(run_id)?;
    let Some(claim_req) = claim_request_for_turn(&initial_turn, run_id) else {
        return Ok(initial_turn);
    };
    let _ = crate::port::store::RuntimeStorePort::claim_lease(store, claim_req)?;
    store.load_runtime_turn(run_id)
}

fn claim_request_for_turn(turn: &RuntimeTurnContext, run_id: &RunId) -> Option<ClaimLeaseReq> {
    turn.snapshot
        .active_lease_id
        .is_none()
        .then(|| ClaimLeaseReq {
            work_id: turn.snapshot.work_id.clone(),
            agent_id: turn.agent_id.clone(),
            lease_id: runtime_lease_id(run_id),
        })
}

fn run_turn_base(turn: &RuntimeTurnContext) -> RunTurnBase {
    RunTurnBase {
        work_id: turn.snapshot.work_id.to_string(),
        agent_id: turn.agent_id.to_string(),
        pending_wake_count: turn.pending_wake.as_ref().map_or(0, |wake| wake.count),
    }
}

fn resume_and_commit_turn(
    store: &(impl RuntimeStorePort + crate::port::store::CommandStorePort),
    runtime: &impl RuntimePort,
    req: &RunTurnOnceReq,
    turn: &RuntimeTurnContext,
) -> Result<CommittedRunTurn, StoreError> {
    let resume = handle_resume_session(
        store,
        runtime,
        ResumeSessionCmd {
            run_id: req.run_id.clone(),
            cwd: req.cwd.clone(),
        },
    )?;
    let context = store.load_context(&turn.snapshot.work_id)?;
    let evidence =
        collect_runtime_observation_evidence(store, &resume.observations, &resume.intent)?;
    let commit = commit_runtime_intent(store, &context, &resume.intent, evidence.clone())?;

    Ok(CommittedRunTurn {
        resume,
        evidence,
        commit,
    })
}

fn run_finalization(
    intent_kind: TransitionKind,
    outcome: DecisionOutcome,
    summary: &str,
) -> RunFinalization {
    match outcome {
        DecisionOutcome::Accepted | DecisionOutcome::OverrideAccepted => {
            if matches!(
                intent_kind,
                TransitionKind::Complete | TransitionKind::Block
            ) {
                RunFinalization::MarkCompleted
            } else {
                RunFinalization::KeepRunning
            }
        }
        DecisionOutcome::Rejected => RunFinalization::MarkFailed {
            reason: "decision_rejected",
            error: StoreError {
                kind: StoreErrorKind::Conflict,
                message: summary.to_owned(),
            },
        },
        DecisionOutcome::Conflict => RunFinalization::MarkFailed {
            reason: "decision_conflict",
            error: StoreError {
                kind: StoreErrorKind::Conflict,
                message: summary.to_owned(),
            },
        },
    }
}

fn apply_run_finalization(
    store: &impl RuntimeStorePort,
    run_id: &RunId,
    finalization: RunFinalization,
) -> Result<(), StoreError> {
    match finalization {
        RunFinalization::KeepRunning => Ok(()),
        RunFinalization::MarkCompleted => store.mark_run_completed(run_id),
        RunFinalization::MarkFailed { reason, error } => {
            store.mark_run_failed(run_id, reason)?;
            Err(error)
        }
    }
}

fn run_turn_response(
    run_id: &RunId,
    turn_base: RunTurnBase,
    committed_turn: CommittedRunTurn,
) -> RunTurnOnceResp {
    RunTurnOnceResp {
        runtime_policy: RUNTIME_RESUME_POLICY,
        run_id: run_id.to_string(),
        work_id: turn_base.work_id,
        agent_id: turn_base.agent_id,
        pending_wake_count: turn_base.pending_wake_count,
        resumed: committed_turn.resume.resumed,
        repair_count: committed_turn.resume.repair_count,
        session_reset_reason: committed_turn.resume.session_reset_reason,
        runtime_session_id: committed_turn.resume.runtime_session_id,
        intent_kind: committed_turn.resume.intent_kind,
        changed_file_count: committed_turn.evidence.changed_files.len(),
        command_result_count: committed_turn.evidence.command_results.len(),
        observed_agent_status: committed_turn.evidence.observed_agent_status,
    }
}

pub(crate) fn runtime_lease_id(run_id: &RunId) -> LeaseId {
    let mut hex = run_id
        .as_str()
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if hex.len() > 12 {
        hex = hex[hex.len() - 12..].to_owned();
    }
    let suffix = format!("{hex:0>12}");
    LeaseId::from(format!("00000000-0000-4000-8000-{suffix}"))
}

pub(crate) fn select_runnable_run_id(candidates: &[QueuedRunCandidate]) -> Option<RunId> {
    candidates
        .iter()
        .filter(|candidate| candidate.agent_status == Some(AgentStatus::Active))
        .filter(|candidate| !candidate.budget_blocked)
        .min_by_key(|candidate| candidate.created_at)
        .map(|candidate| candidate.run_id.clone())
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::{
            coclai::runtime::{CoclaiRuntime, ScriptedReply},
            memory::store::{MemoryStore, DEMO_AGENT_ID, DEMO_DOING_WORK_ID, DEMO_TODO_WORK_ID},
        },
        app::cmd::{
            test_support::{
                load_runtime_assets, runtime_intent_output, sample_usage, RuntimeIntentOutput,
            },
            RUNTIME_RESUME_POLICY,
        },
        model::{
            workspace_fingerprint, AgentStatus, DecisionOutcome, LeaseId, RuntimeKind, SessionId,
            TaskSession, TransitionIntent, TransitionKind, WorkId, WorkPatch,
        },
        port::{
            runtime::RuntimeHandle,
            store::{MergeWakeReq, QueuedRunCandidate, StoreErrorKind, StorePort},
        },
    };

    use super::{handle_run_turn_once, runtime_lease_id, select_runnable_run_id, RunTurnOnceReq};

    #[test]
    fn run_turn_once_response_exposes_work_run_agent_session_and_wake_outcome() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-turn-once".to_owned(),
                },
                raw_output: valid_queued_output(),
                intent: queued_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .merge_wake(MergeWakeReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "scheduler".to_owned(),
                obligations: vec!["follow up".to_owned()],
            })
            .expect("wake should create queued run");

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-2"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("run turn once should succeed");

        assert_eq!(resp.runtime_policy, RUNTIME_RESUME_POLICY);
        assert_eq!(resp.run_id, "run-2");
        assert_eq!(resp.work_id, DEMO_TODO_WORK_ID);
        assert_eq!(resp.agent_id, DEMO_AGENT_ID);
        assert_eq!(resp.pending_wake_count, 1);
        assert!(!resp.resumed);
        assert_eq!(resp.repair_count, 0);
        assert_eq!(resp.session_reset_reason, None);
        assert_eq!(resp.runtime_session_id, "runtime-turn-once");
        assert_eq!(resp.intent_kind, TransitionKind::ProposeProgress);
        assert_eq!(resp.changed_file_count, 0);
        assert_eq!(resp.command_result_count, 0);
        assert_eq!(resp.observed_agent_status, Some(AgentStatus::Active));
        assert_eq!(
            store
                .load_context(&WorkId::from(DEMO_TODO_WORK_ID))
                .expect("context should load after claim")
                .lease
                .expect("lease should be claimed before runtime")
                .run_id,
            Some(crate::model::RunId::from("run-2"))
        );
    }

    #[test]
    fn run_turn_once_reports_existing_doing_work_context() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-doing".to_owned(),
                },
                raw_output: valid_doing_output(),
                intent: doing_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("doing run turn should succeed");

        assert_eq!(resp.work_id, DEMO_DOING_WORK_ID);
        assert_eq!(resp.pending_wake_count, 0);
        assert_eq!(resp.intent_kind, TransitionKind::ProposeProgress);
    }

    #[test]
    fn run_turn_once_commits_runtime_observations_without_workspace_port() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-cutover".to_owned(),
                },
                raw_output: valid_queued_output(),
                intent: queued_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .merge_wake(MergeWakeReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "scheduler".to_owned(),
                obligations: vec!["follow up".to_owned()],
            })
            .expect("wake should create queued run");

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-2"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("runtime observations should close the turn without workspace");

        assert_eq!(resp.run_id, "run-2");
    }

    #[test]
    fn run_turn_once_reports_standard_stage_evidence_counts_for_complete_intent() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-complete".to_owned(),
                },
                raw_output: valid_complete_output(),
                intent: complete_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("complete run turn should succeed");

        assert_eq!(resp.intent_kind, TransitionKind::Complete);
        assert_eq!(resp.changed_file_count, 1);
        assert_eq!(resp.command_result_count, 3);
        assert_eq!(resp.observed_agent_status, Some(AgentStatus::Active));
    }

    #[test]
    fn run_turn_once_commits_runtime_intent_into_snapshot_and_transition_record() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-commit".to_owned(),
                },
                raw_output: valid_complete_output(),
                intent: complete_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        let before = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load before turn")
            .snapshot;
        let before_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load before turn")
            .len();

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("run turn once should commit");

        let after = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load after turn")
            .snapshot;
        let after_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load after turn");

        assert_eq!(resp.intent_kind, TransitionKind::Complete);
        assert_eq!(before.status, crate::model::WorkStatus::Doing);
        assert_eq!(after.status, crate::model::WorkStatus::Done);
        assert_eq!(after.rev, before.rev + 1);
        assert_eq!(after_records.len(), before_records + 1);
        assert_eq!(
            after_records
                .last()
                .expect("record should be appended")
                .kind,
            TransitionKind::Complete
        );
        assert_eq!(
            after_records
                .last()
                .expect("record should be appended")
                .outcome,
            DecisionOutcome::Accepted
        );
        assert_eq!(
            after_records
                .last()
                .expect("record should be appended")
                .run_id
                .as_ref()
                .map(|run_id| run_id.as_str()),
            Some("run-1")
        );
        assert_eq!(
            after_records
                .last()
                .expect("record should be appended")
                .session_id
                .as_ref()
                .map(|session_id| session_id.as_str()),
            Some("session-run-1")
        );
        let run = store
            .read_run(&crate::model::RunId::from("run-1"))
            .expect("run read should succeed");
        assert_eq!(run.status, "completed");
    }

    #[test]
    fn run_turn_once_rejected_decision_marks_run_failed_after_record_append() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-rejected".to_owned(),
                },
                raw_output: rejected_complete_output(),
                intent: rejected_complete_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        let before_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load before turn")
            .len();

        let error = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect_err("gate rejection should fail turn");

        let after_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load after turn");
        let run = store
            .read_run(&crate::model::RunId::from("run-1"))
            .expect("run read should succeed");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert_eq!(run.status, "failed");
        assert_eq!(after_records.len(), before_records + 1);
        assert_eq!(
            after_records.last().expect("record should append").outcome,
            DecisionOutcome::Rejected
        );
    }

    #[test]
    fn run_turn_once_conflict_write_marks_run_failed() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-conflict".to_owned(),
                },
                raw_output: valid_stale_doing_output(),
                intent: stale_doing_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        let before_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load before turn")
            .len();

        let error = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect_err("conflict should fail turn");

        let after_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load after turn");
        let run = store
            .read_run(&crate::model::RunId::from("run-1"))
            .expect("run read should succeed");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert_eq!(run.status, "failed");
        assert_eq!(after_records.len(), before_records);
    }

    #[test]
    fn run_turn_once_prefers_existing_matching_session() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-existing".to_owned(),
                },
                raw_output: valid_doing_output(),
                intent: doing_intent(),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-existing"),
                company_id: crate::model::CompanyId::from("00000000-0000-4000-8000-000000000001"),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-existing".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: Some("accepted earlier".to_owned()),
                last_gate_summary: Some("gate ok".to_owned()),
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            })
            .expect("session should seed");

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("run turn once should resume");

        assert!(resp.resumed);
        assert_eq!(resp.repair_count, 0);
        assert_eq!(resp.session_reset_reason, None);
        assert_eq!(resp.runtime_session_id, "runtime-existing");
    }

    #[test]
    fn run_turn_once_reports_invalid_session_single_retry() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                ScriptedReply {
                    handle: RuntimeHandle {
                        runtime_session_id: "runtime-old".to_owned(),
                    },
                    raw_output: valid_doing_output(),
                    intent: doing_intent(),
                    usage: sample_usage(120, 48, 3, 7),
                    invalid_session: true,
                },
                ScriptedReply {
                    handle: RuntimeHandle {
                        runtime_session_id: "runtime-fresh".to_owned(),
                    },
                    raw_output: valid_doing_output(),
                    intent: doing_intent(),
                    usage: sample_usage(120, 48, 3, 7),
                    invalid_session: false,
                },
            ],
        );
        let store = MemoryStore::demo();
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-existing"),
                company_id: crate::model::CompanyId::from("00000000-0000-4000-8000-000000000001"),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-old".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: Some("accepted earlier".to_owned()),
                last_gate_summary: Some("gate ok".to_owned()),
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            })
            .expect("session should seed");

        let resp = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("run turn once should recover from invalid session");

        assert!(!resp.resumed);
        assert_eq!(resp.repair_count, 0);
        assert_eq!(
            resp.session_reset_reason,
            Some(crate::model::SessionInvalidationReason::Runtime)
        );
        assert_eq!(resp.runtime_session_id, "runtime-fresh");
    }

    #[test]
    fn run_turn_once_invalid_output_does_not_mutate_snapshot_or_append_transition_record() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                ScriptedReply {
                    handle: RuntimeHandle {
                        runtime_session_id: "runtime-bad".to_owned(),
                    },
                    raw_output: "{\"kind\":\"complete\"}".to_owned(),
                    intent: doing_intent(),
                    usage: sample_usage(120, 48, 3, 7),
                    invalid_session: false,
                },
                ScriptedReply {
                    handle: RuntimeHandle {
                        runtime_session_id: "runtime-bad".to_owned(),
                    },
                    raw_output: "{\"kind\":\"complete\"}".to_owned(),
                    intent: doing_intent(),
                    usage: sample_usage(120, 48, 3, 7),
                    invalid_session: false,
                },
            ],
        );
        let store = MemoryStore::demo();
        let before = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load before runtime")
            .snapshot;
        let before_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load before runtime")
            .len();

        let error = handle_run_turn_once(
            &store,
            &runtime,
            RunTurnOnceReq {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect_err("invalid output should fail turn");

        let after = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load after runtime failure")
            .snapshot;
        let after_records = store
            .load_transition_records(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("transition records should load after runtime failure")
            .len();
        let run = store
            .read_run(&crate::model::RunId::from("run-1"))
            .expect("run read should succeed");

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Unavailable);
        assert_eq!(after, before);
        assert_eq!(after_records, before_records);
        assert_eq!(run.status, "failed");
    }

    #[test]
    fn select_runnable_run_id_prefers_oldest_active_candidate() {
        let candidates = vec![
            QueuedRunCandidate {
                run_id: crate::model::RunId::from("run-paused"),
                agent_status: Some(AgentStatus::Paused),
                budget_blocked: false,
                created_at: std::time::SystemTime::UNIX_EPOCH,
            },
            QueuedRunCandidate {
                run_id: crate::model::RunId::from("run-2"),
                agent_status: Some(AgentStatus::Active),
                budget_blocked: false,
                created_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
            },
            QueuedRunCandidate {
                run_id: crate::model::RunId::from("run-1"),
                agent_status: Some(AgentStatus::Active),
                budget_blocked: false,
                created_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            },
        ];

        assert_eq!(
            select_runnable_run_id(&candidates),
            Some(crate::model::RunId::from("run-1"))
        );
    }

    #[test]
    fn select_runnable_run_id_skips_budget_blocked_candidates() {
        let candidates = vec![
            QueuedRunCandidate {
                run_id: crate::model::RunId::from("run-blocked"),
                agent_status: Some(AgentStatus::Active),
                budget_blocked: true,
                created_at: std::time::SystemTime::UNIX_EPOCH,
            },
            QueuedRunCandidate {
                run_id: crate::model::RunId::from("run-ready"),
                agent_status: Some(AgentStatus::Active),
                budget_blocked: false,
                created_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            },
        ];

        assert_eq!(
            select_runnable_run_id(&candidates),
            Some(crate::model::RunId::from("run-ready"))
        );
    }

    fn queued_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_TODO_WORK_ID),
            agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            lease_id: runtime_lease_id(&crate::model::RunId::from("run-2")),
            expected_rev: 1,
            kind: TransitionKind::ProposeProgress,
            patch: WorkPatch {
                summary: "scheduler turn".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: vec![crate::model::ProofHint {
                kind: crate::model::ProofHintKind::Summary,
                value: "scheduler turn".to_owned(),
            }],
        }
    }

    fn doing_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from("00000000-0000-4000-8000-000000000013"),
            expected_rev: 1,
            kind: TransitionKind::ProposeProgress,
            patch: WorkPatch {
                summary: "doing turn".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: Vec::new(),
        }
    }

    fn complete_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from("00000000-0000-4000-8000-000000000013"),
            expected_rev: 1,
            kind: TransitionKind::Complete,
            patch: WorkPatch {
                summary: "complete with evidence".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: Some("done".to_owned()),
            proof_hints: vec![crate::model::ProofHint {
                kind: crate::model::ProofHintKind::File,
                value: "src/kernel/mod.rs".to_owned(),
            }],
        }
    }

    fn rejected_complete_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from("00000000-0000-4000-8000-000000000013"),
            expected_rev: 1,
            kind: TransitionKind::Complete,
            patch: WorkPatch {
                summary: "complete without file evidence".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: Some("done".to_owned()),
            proof_hints: vec![crate::model::ProofHint {
                kind: crate::model::ProofHintKind::Summary,
                value: "complete without file evidence".to_owned(),
            }],
        }
    }

    fn stale_doing_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from("00000000-0000-4000-8000-000000000013"),
            expected_rev: 0,
            kind: TransitionKind::ProposeProgress,
            patch: WorkPatch {
                summary: "stale doing turn".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: vec![crate::model::ProofHint {
                kind: crate::model::ProofHintKind::Summary,
                value: "stale doing turn".to_owned(),
            }],
        }
    }

    fn valid_queued_output() -> String {
        runtime_intent_output(RuntimeIntentOutput {
            work_id: DEMO_TODO_WORK_ID,
            agent_id: DEMO_AGENT_ID,
            lease_id: runtime_lease_id(&crate::model::RunId::from("run-2")).as_str(),
            expected_rev: 1,
            kind: "propose_progress",
            summary: "scheduler turn",
            note: None,
            proof_hints: &[("summary", "scheduler turn")],
        })
    }

    fn valid_doing_output() -> String {
        runtime_intent_output(RuntimeIntentOutput {
            work_id: DEMO_DOING_WORK_ID,
            agent_id: DEMO_AGENT_ID,
            lease_id: "00000000-0000-4000-8000-000000000013",
            expected_rev: 1,
            kind: "propose_progress",
            summary: "doing turn",
            note: None,
            proof_hints: &[],
        })
    }

    fn valid_complete_output() -> String {
        runtime_intent_output(RuntimeIntentOutput {
            work_id: DEMO_DOING_WORK_ID,
            agent_id: DEMO_AGENT_ID,
            lease_id: "00000000-0000-4000-8000-000000000013",
            expected_rev: 1,
            kind: "complete",
            summary: "complete with evidence",
            note: Some("done"),
            proof_hints: &[("file", "src/kernel/mod.rs")],
        })
    }

    fn rejected_complete_output() -> String {
        runtime_intent_output(RuntimeIntentOutput {
            work_id: DEMO_DOING_WORK_ID,
            agent_id: DEMO_AGENT_ID,
            lease_id: "00000000-0000-4000-8000-000000000013",
            expected_rev: 1,
            kind: "complete",
            summary: "complete without file evidence",
            note: Some("done"),
            proof_hints: &[("summary", "complete without file evidence")],
        })
    }

    fn valid_stale_doing_output() -> String {
        runtime_intent_output(RuntimeIntentOutput {
            work_id: DEMO_DOING_WORK_ID,
            agent_id: DEMO_AGENT_ID,
            lease_id: "00000000-0000-4000-8000-000000000013",
            expected_rev: 0,
            kind: "propose_progress",
            summary: "stale doing turn",
            note: None,
            proof_hints: &[("summary", "stale doing turn")],
        })
    }
}
