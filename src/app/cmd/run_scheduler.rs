use serde::Serialize;

use crate::port::{
    runtime::RuntimePort,
    store::{QueuedRunCandidate, RuntimeStorePort, SchedulerStorePort, StoreError},
};

use super::{
    run_turn_once::{handle_run_turn_once, select_runnable_run_id, RunTurnOnceReq},
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
    store: &(impl SchedulerStorePort + RuntimeStorePort + crate::port::store::CommandStorePort),
    runtime: &impl RuntimePort,
    cmd: RunSchedulerCmd,
) -> Result<RunSchedulerAck, StoreError> {
    store.reap_timed_out_runs(RUN_REAPER_TIMEOUT)?;

    let queued_runs = store.load_queued_runs()?;
    let Some(run_id) = select_runnable_run_id(&queued_runs) else {
        return Ok(RunSchedulerAck {
            queue_policy: SCHEDULER_PICK_POLICY,
            run_id: None,
            runtime_session_id: None,
        });
    };

    let ack = handle_run_turn_once(
        store,
        runtime,
        RunTurnOnceReq {
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
    select_runnable_run_id(candidates)
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::{
            coclai::runtime::{CoclaiRuntime, ScriptedReply},
            memory::store::{MemoryStore, DEMO_AGENT_ID, DEMO_DOING_WORK_ID, DEMO_TODO_WORK_ID},
        },
        app::cmd::test_support::{
            load_runtime_assets, runtime_intent_output, sample_usage, RuntimeIntentOutput,
        },
        model::{
            ActorId, ActorKind, AgentId, AgentStatus, BillingKind, CompanyId, ConsumptionUsage,
            ContractSetId, TransitionIntent, TransitionKind, WorkId, WorkKind, WorkPatch,
            WorkStatus,
        },
        port::{
            runtime::RuntimeHandle,
            store::{
                ActivateContractReq, CreateAgentReq, CreateCompanyReq, CreateContractDraftReq,
                CreateWorkReq, MergeWakeReq, RecordConsumptionReq, SessionKey, StorePort,
            },
        },
    };

    use crate::app::cmd::run_turn_once::runtime_lease_id;

    use super::{handle_run_scheduler, next_queued_run_id, RunSchedulerCmd};

    #[test]
    fn run_scheduler_consumes_oldest_queued_run_and_saves_session() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-scheduler".to_owned(),
                },
                raw_output: valid_output(
                    DEMO_TODO_WORK_ID,
                    runtime_lease_id(&crate::model::RunId::from("run-2")).as_str(),
                    1,
                    "scheduler turn",
                ),
                intent: intent(),
                usage: sample_usage(90, 32, 2, 5),
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
        let assets = load_runtime_assets();
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
    fn run_scheduler_skips_budget_blocked_queue_before_runtime_start() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(assets, Vec::new());
        let store = MemoryStore::demo();
        let company = store
            .create_company(CreateCompanyReq {
                name: "Budget Labs".to_owned(),
                description: "company hard stop".to_owned(),
                runtime_hard_stop_cents: Some(5),
            })
            .expect("company create should succeed");
        let rules = store.read_contracts().rules;
        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "budget-contract".to_owned(),
                rules,
            })
            .expect("contract draft should succeed");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("contract should activate");
        let contract_set_id = store
            .read_companies()
            .items
            .into_iter()
            .find(|item| item.company_id == company.profile.company_id.as_str())
            .and_then(|item| item.active_contract_set_id)
            .expect("new company should expose active contract");
        let agent = store
            .create_agent(CreateAgentReq {
                company_id: company.profile.company_id.clone(),
                name: "Budget Agent".to_owned(),
                role: "operator".to_owned(),
            })
            .expect("agent create should succeed");
        let work = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Blocked work".to_owned(),
                body: "budget check".to_owned(),
                contract_set_id: ContractSetId::from(contract_set_id),
            })
            .expect("work create should succeed");
        store
            .merge_wake(MergeWakeReq {
                work_id: work.snapshot.work_id.clone(),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "budget check".to_owned(),
                obligations: vec!["stay queued".to_owned()],
            })
            .expect("wake should create queued run");
        let queued_run = store
            .load_queued_runs()
            .expect("queued runs should load")
            .into_iter()
            .find(|candidate| candidate.agent_status == Some(AgentStatus::Active))
            .expect("queued run should exist");
        store
            .record_consumption(RecordConsumptionReq {
                company_id: CompanyId::from(company.profile.company_id.as_str()),
                agent_id: agent.agent_id.clone(),
                run_id: queued_run.run_id.clone(),
                billing_kind: BillingKind::Api,
                usage: ConsumptionUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    run_seconds: 1,
                    estimated_cost_cents: Some(5),
                },
            })
            .expect("consumption should persist");

        let blocked_run = store
            .load_queued_runs()
            .expect("queued runs should load")
            .into_iter()
            .find(|candidate| candidate.run_id == queued_run.run_id)
            .expect("queued run should still exist");
        let ack = handle_run_scheduler(
            &store,
            &runtime,
            RunSchedulerCmd {
                cwd: "/repo".to_owned(),
            },
        )
        .expect("scheduler should skip budget blocked run");

        assert!(blocked_run.budget_blocked);
        assert!(ack.run_id.is_none());
        assert!(ack.runtime_session_id.is_none());
    }

    #[test]
    fn run_scheduler_reaps_stale_running_run_then_consumes_follow_up_queue() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-reaped".to_owned(),
                },
                raw_output: valid_output(
                    DEMO_DOING_WORK_ID,
                    runtime_lease_id(&crate::model::RunId::from("run-2")).as_str(),
                    3,
                    "reaped scheduler turn",
                ),
                intent: doing_work_intent(),
                usage: sample_usage(90, 32, 2, 5),
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
        assert_eq!(
            work.items[0].active_lease_id.as_deref(),
            Some(runtime_lease_id(&crate::model::RunId::from("run-2")).as_str())
        );
        assert_eq!(work.items[0].status, WorkStatus::Doing);
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
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-oldest-queue".to_owned(),
                },
                raw_output: valid_output(
                    DEMO_TODO_WORK_ID,
                    runtime_lease_id(&crate::model::RunId::from("run-2")).as_str(),
                    1,
                    "scheduler turn",
                ),
                intent: intent(),
                usage: sample_usage(90, 32, 2, 5),
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

    fn doing_work_intent() -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            lease_id: runtime_lease_id(&crate::model::RunId::from("run-2")),
            expected_rev: 3,
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
        runtime_intent_output(RuntimeIntentOutput {
            work_id,
            agent_id: DEMO_AGENT_ID,
            lease_id,
            expected_rev,
            kind: "propose_progress",
            summary,
            note: None,
            proof_hints: &[("summary", summary)],
        })
    }
}
