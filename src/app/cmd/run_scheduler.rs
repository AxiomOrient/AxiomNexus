use serde::Serialize;

use crate::port::{
    runtime::RuntimePort,
    store::{QueuedRunCandidate, RuntimeStorePort, SchedulerStorePort, StoreError},
};

use super::{
    resume_session::{handle_resume_session, ResumeSessionCmd},
    RUN_REAPER_TIMEOUT, SCHEDULER_PICK_POLICY,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunSchedulerCmd {
    pub(crate) cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RunSchedulerAck {
    pub(crate) queue_policy: &'static str,
    pub(crate) run_id: Option<String>,
    pub(crate) runtime_session_id: Option<String>,
}

pub(crate) fn handle_run_scheduler(
    store: &(impl SchedulerStorePort + RuntimeStorePort),
    runtime: &impl RuntimePort,
    cmd: RunSchedulerCmd,
) -> Result<RunSchedulerAck, StoreError> {
    store.reap_timed_out_runs(RUN_REAPER_TIMEOUT)?;

    let queued_runs = store.load_queued_runs()?;
    let Some(run_id) = next_queued_run_id(&queued_runs) else {
        return Ok(RunSchedulerAck {
            queue_policy: SCHEDULER_PICK_POLICY,
            run_id: None,
            runtime_session_id: None,
        });
    };

    let ack = handle_resume_session(
        store,
        runtime,
        ResumeSessionCmd {
            run_id: run_id.clone(),
            cwd: cmd.cwd,
        },
    )?;

    Ok(RunSchedulerAck {
        queue_policy: SCHEDULER_PICK_POLICY,
        run_id: Some(run_id.to_string()),
        runtime_session_id: Some(ack.runtime_session_id),
    })
}

pub(crate) fn next_queued_run_id(candidates: &[QueuedRunCandidate]) -> Option<crate::model::RunId> {
    candidates
        .iter()
        .filter(|candidate| candidate.agent_status == Some(crate::model::AgentStatus::Active))
        .min_by_key(|candidate| candidate.created_at)
        .map(|candidate| candidate.run_id.clone())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        adapter::{
            coclai::{
                assets::RuntimeAssets,
                runtime::{CoclaiRuntime, ScriptedReply},
            },
            memory::store::{
                MemoryStore, DEMO_AGENT_ID, DEMO_DOING_WORK_ID, DEMO_LEASE_ID, DEMO_TODO_WORK_ID,
            },
        },
        model::{
            AgentId, ConsumptionUsage, LeaseId, TransitionIntent, TransitionKind, WorkId,
            WorkPatch, WorkStatus,
        },
        port::{
            runtime::RuntimeHandle,
            store::{MergeWakeReq, SessionKey, StorePort},
        },
    };

    use super::{handle_run_scheduler, next_queued_run_id, RunSchedulerCmd};

    #[test]
    fn run_scheduler_consumes_oldest_queued_run_and_saves_session() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-scheduler".to_owned(),
                },
                raw_output: valid_output(
                    DEMO_TODO_WORK_ID,
                    "00000000-0000-4000-8000-000000000013",
                    0,
                    "scheduler turn",
                ),
                intent: intent(),
                usage: usage(),
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

        let ack = handle_run_scheduler(
            &store,
            &runtime,
            RunSchedulerCmd {
                cwd: "/repo".to_owned(),
            },
        )
        .expect("scheduler should consume queued run");

        let session = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
            })
            .expect("session lookup should work")
            .expect("session should be saved");

        assert_eq!(ack.run_id.as_deref(), Some("run-2"));
        assert_eq!(ack.runtime_session_id.as_deref(), Some("runtime-scheduler"));
        assert_eq!(session.runtime_session_id, "runtime-scheduler");
    }

    #[test]
    fn run_scheduler_is_idle_when_no_queued_run_exists() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(assets, Vec::new());
        let store = MemoryStore::demo();

        let ack = handle_run_scheduler(
            &store,
            &runtime,
            RunSchedulerCmd {
                cwd: "/repo".to_owned(),
            },
        )
        .expect("idle scheduler should not fail");

        assert!(ack.run_id.is_none());
        assert!(ack.runtime_session_id.is_none());
    }

    #[test]
    fn run_scheduler_reaps_stale_running_run_then_consumes_follow_up_queue() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-reaped".to_owned(),
                },
                raw_output: valid_output(
                    DEMO_DOING_WORK_ID,
                    DEMO_LEASE_ID,
                    2,
                    "reaped scheduler turn",
                ),
                intent: doing_work_intent(),
                usage: usage(),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .merge_wake(MergeWakeReq {
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "re-run after timeout".to_owned(),
                obligations: vec!["retry".to_owned()],
            })
            .expect("wake should remain pending until the reaper releases the lease");

        let ack = handle_run_scheduler(
            &store,
            &runtime,
            RunSchedulerCmd {
                cwd: "/repo".to_owned(),
            },
        )
        .expect("scheduler should reap stale run and consume queued follow-up");

        let work = store
            .read_work(Some(&WorkId::from(DEMO_DOING_WORK_ID)))
            .expect("work read should succeed");
        let session = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
            })
            .expect("session lookup should work")
            .expect("session should be saved");

        assert_eq!(ack.run_id.as_deref(), Some("run-2"));
        assert_eq!(ack.runtime_session_id.as_deref(), Some("runtime-reaped"));
        assert_eq!(session.runtime_session_id, "runtime-reaped");
        assert_eq!(work.items[0].active_lease_id, None);
        assert_eq!(work.items[0].status, WorkStatus::Todo);
        assert_eq!(work.items[0].comments.len(), 2);
        assert_eq!(
            work.items[0].comments[0].author_kind,
            crate::model::ActorKind::Board
        );
        assert!(work.items[0].comments[0]
            .body
            .contains("manual wake merged"));
        assert_eq!(
            work.items[0].comments[1].author_kind,
            crate::model::ActorKind::System
        );
        assert_eq!(
            work.items[0].comments[1].source.as_deref(),
            Some("scheduler")
        );
        assert!(work.items[0].comments[1]
            .body
            .contains("timed out run run-1"));
    }

    #[test]
    fn run_scheduler_prefers_existing_oldest_queue_over_new_reaper_follow_up() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-oldest-queue".to_owned(),
                },
                raw_output: valid_output(
                    DEMO_TODO_WORK_ID,
                    "00000000-0000-4000-8000-000000000013",
                    0,
                    "scheduler turn",
                ),
                intent: intent(),
                usage: usage(),
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
                reason: "older queued wake".to_owned(),
                obligations: vec!["todo follow up".to_owned()],
            })
            .expect("todo wake should create the oldest queued run");
        store
            .merge_wake(MergeWakeReq {
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "re-run after timeout".to_owned(),
                obligations: vec!["retry".to_owned()],
            })
            .expect("doing wake should remain pending until the reaper releases the lease");

        let ack = handle_run_scheduler(
            &store,
            &runtime,
            RunSchedulerCmd {
                cwd: "/repo".to_owned(),
            },
        )
        .expect("scheduler should prefer the oldest queued run");

        let todo_session = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
            })
            .expect("session lookup should work")
            .expect("todo session should be saved");
        let next_queued_run = next_queued_run_id(
            &store
                .load_queued_runs()
                .expect("queued run lookup should work"),
        );
        let doing_work = store
            .read_work(Some(&WorkId::from(DEMO_DOING_WORK_ID)))
            .expect("work read should succeed");

        assert_eq!(ack.run_id.as_deref(), Some("run-2"));
        assert_eq!(
            ack.runtime_session_id.as_deref(),
            Some("runtime-oldest-queue")
        );
        assert_eq!(todo_session.runtime_session_id, "runtime-oldest-queue");
        assert_eq!(next_queued_run, Some(crate::model::RunId::from("run-3")));
        assert_eq!(doing_work.items[0].comments.len(), 2);
        assert_eq!(
            doing_work.items[0].comments[1].author_kind,
            crate::model::ActorKind::System
        );
        assert_eq!(
            doing_work.items[0].comments[1].source.as_deref(),
            Some("scheduler")
        );
        assert!(doing_work.items[0].comments[1]
            .body
            .contains("follow-up run-3 queued"));
    }

    fn intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_TODO_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from("00000000-0000-4000-8000-000000000013"),
            expected_rev: 0,
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

    fn doing_work_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from(DEMO_LEASE_ID),
            expected_rev: 2,
            kind: TransitionKind::ProposeProgress,
            patch: WorkPatch {
                summary: "reaped scheduler turn".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: vec![crate::model::ProofHint {
                kind: crate::model::ProofHintKind::Summary,
                value: "reaped scheduler turn".to_owned(),
            }],
        }
    }

    fn valid_output(work_id: &str, lease_id: &str, expected_rev: u64, summary: &str) -> String {
        format!(
            "{{\"work_id\":\"{work_id}\",\"agent_id\":\"{agent_id}\",\"lease_id\":\"{lease_id}\",\"expected_rev\":{expected_rev},\"kind\":\"propose_progress\",\"patch\":{{\"summary\":\"{summary}\",\"resolved_obligations\":[],\"declared_risks\":[]}},\"note\":null,\"proof_hints\":[{{\"kind\":\"summary\",\"value\":\"{summary}\"}}]}}",
            work_id = work_id,
            agent_id = DEMO_AGENT_ID,
            lease_id = lease_id,
            expected_rev = expected_rev,
            summary = summary,
        )
    }

    fn usage() -> ConsumptionUsage {
        ConsumptionUsage {
            input_tokens: 90,
            output_tokens: 32,
            run_seconds: 2,
            estimated_cost_cents: Some(5),
        }
    }
}
