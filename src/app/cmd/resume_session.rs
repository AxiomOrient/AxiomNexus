use serde::Serialize;

use crate::{
    kernel,
    model::{
        workspace_fingerprint, ActorKind, BillingKind, GateSpec, RunId, RuntimeKind, SessionId,
        TaskSession, TransitionIntent, TransitionKind, WorkStatus,
    },
    port::{
        runtime::{ExecuteTurnReq, GateCommandSpec, RuntimeObservations, RuntimePort},
        store::{
            RecordConsumptionReq, RuntimeStorePort, RuntimeTurnContext, SessionKey, StoreError,
        },
    },
};

use super::RUNTIME_RESUME_POLICY;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResumeSessionCmd {
    pub(crate) run_id: RunId,
    pub(crate) cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ResumeSessionAck {
    pub(crate) runtime_policy: &'static str,
    pub(crate) resumed: bool,
    pub(crate) repair_count: u8,
    pub(crate) session_reset_reason: Option<crate::model::SessionInvalidationReason>,
    pub(crate) runtime_session_id: String,
    #[serde(skip_serializing)]
    pub(crate) intent: TransitionIntent,
    #[serde(skip_serializing)]
    pub(crate) observations: RuntimeObservations,
    pub(crate) intent_kind: TransitionKind,
}

pub(crate) fn handle_resume_session(
    store: &impl RuntimeStorePort,
    runtime: &impl RuntimePort,
    cmd: ResumeSessionCmd,
) -> Result<ResumeSessionAck, StoreError> {
    let turn = store.load_runtime_turn(&cmd.run_id)?;
    let cwd = cmd.cwd;
    let session_key = SessionKey {
        agent_id: turn.agent_id.clone(),
        work_id: turn.snapshot.work_id.clone(),
    };
    let existing_session = store.load_session(&session_key)?;
    let local_reset_reason = existing_session.as_ref().and_then(|session| {
        kernel::session_invalidation_reason(
            session,
            &turn.agent_id,
            &turn.snapshot.work_id,
            &cwd,
            RuntimeKind::Coclai,
        )
    });
    let resume_session = if local_reset_reason.is_none() {
        existing_session.clone()
    } else {
        None
    };
    let prompt_input = prompt_input_for(&turn, resume_session.as_ref());
    let outcome = match runtime.execute_turn(ExecuteTurnReq {
        session_key,
        cwd: cwd.clone(),
        existing_session: resume_session.clone(),
        prompt_input,
        gate_plan: gate_plan_for(&turn.contract, turn.snapshot.status),
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            store.mark_run_failed(&turn.run_id, "runtime_failure")?;
            return Err(runtime_error_to_store_error(error));
        }
    };
    let session_reset_reason = local_reset_reason.or(outcome.session_reset_reason);
    let session_base = if session_reset_reason.is_none() {
        resume_session.as_ref()
    } else {
        None
    };

    store.save_session(&session_from_turn(
        &turn,
        session_base,
        &outcome.handle.runtime_session_id,
        &cwd,
    ))?;
    store.mark_run_running(&turn.run_id)?;
    store.record_consumption(RecordConsumptionReq {
        company_id: turn.snapshot.company_id.clone(),
        agent_id: turn.agent_id.clone(),
        run_id: turn.run_id.clone(),
        billing_kind: BillingKind::Api,
        usage: outcome.result.usage.clone(),
    })?;

    let intent_kind = outcome.result.intent.kind;

    Ok(ResumeSessionAck {
        runtime_policy: RUNTIME_RESUME_POLICY,
        resumed: outcome.resumed,
        repair_count: outcome.repair_count,
        session_reset_reason,
        runtime_session_id: outcome.handle.runtime_session_id,
        intent: outcome.result.intent,
        observations: outcome.observations,
        intent_kind,
    })
}

fn prompt_input_for(
    turn: &RuntimeTurnContext,
    existing_session: Option<&TaskSession>,
) -> crate::port::runtime::PromptEnvelopeInput {
    crate::port::runtime::PromptEnvelopeInput {
        snapshot: turn.snapshot.clone(),
        unresolved_obligations: turn
            .pending_wake
            .as_ref()
            .map(|wake| wake.obligation_json.iter().cloned().collect())
            .unwrap_or_default(),
        contract_summary: format!(
            "{} rev={} rules={}",
            turn.contract.name,
            turn.contract.revision,
            turn.contract.rules.len()
        ),
        last_gate_summary: existing_session.and_then(|session| session.last_gate_summary.clone()),
        last_decision_summary: existing_session
            .and_then(|session| session.last_decision_summary.clone()),
    }
}

fn gate_plan_for(contract: &crate::model::ContractSet, status: WorkStatus) -> Vec<GateCommandSpec> {
    contract
        .rules
        .iter()
        .filter(|rule| rule.actor_kind == ActorKind::Agent)
        .filter(|rule| rule.kind.is_runtime_intent())
        .filter(|rule| rule.from.contains(&status))
        .flat_map(|rule| {
            rule.gates.iter().filter_map(|gate| match gate {
                GateSpec::CommandSucceeds {
                    argv,
                    timeout_sec,
                    allow_exit_codes,
                } => Some(GateCommandSpec {
                    applies_to_kind: rule.kind,
                    argv: argv.clone(),
                    timeout_sec: *timeout_sec,
                    allow_exit_codes: allow_exit_codes.clone(),
                }),
                _ => None,
            })
        })
        .collect()
}

fn session_from_turn(
    turn: &RuntimeTurnContext,
    existing_session: Option<&TaskSession>,
    runtime_session_id: &str,
    cwd: &str,
) -> TaskSession {
    TaskSession {
        session_id: existing_session
            .map(|session| session.session_id.clone())
            .unwrap_or_else(|| SessionId::from(format!("session-{}", turn.run_id))),
        company_id: turn.snapshot.company_id.clone(),
        agent_id: turn.agent_id.clone(),
        work_id: turn.snapshot.work_id.clone(),
        runtime: RuntimeKind::Coclai,
        runtime_session_id: runtime_session_id.to_owned(),
        cwd: cwd.to_owned(),
        workspace_fingerprint: workspace_fingerprint(cwd),
        contract_rev: turn.contract.revision,
        last_record_id: existing_session.and_then(|session| session.last_record_id.clone()),
        last_decision_summary: existing_session
            .and_then(|session| session.last_decision_summary.clone()),
        last_gate_summary: existing_session.and_then(|session| session.last_gate_summary.clone()),
        updated_at: turn.snapshot.updated_at,
    }
}

fn runtime_error_to_store_error(error: crate::port::runtime::RuntimeError) -> StoreError {
    StoreError {
        kind: crate::port::store::StoreErrorKind::Unavailable,
        message: format!("resume_session runtime failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use crate::{
        adapter::{
            coclai::runtime::{CoclaiRuntime, ScriptedReply},
            memory::store::{MemoryStore, DEMO_AGENT_ID, DEMO_DOING_WORK_ID},
        },
        app::cmd::test_support::{
            load_runtime_assets, runtime_intent_output, sample_usage, RuntimeIntentOutput,
        },
        model::{
            workspace_fingerprint, AgentId, CompanyId, LeaseId, RunId, RuntimeKind, SessionId,
            SessionInvalidationReason, TaskSession, TransitionIntent, TransitionKind, WorkId,
            WorkPatch,
        },
        port::{
            runtime::RuntimeHandle,
            store::{SessionKey, StorePort},
        },
    };

    use super::{handle_resume_session, ResumeSessionCmd};

    #[test]
    fn resume_session_starts_for_queued_run_and_saves_session() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-queued".to_owned(),
                },
                raw_output: valid_output("propose_progress"),
                intent: intent(TransitionKind::ProposeProgress),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .merge_wake(crate::port::store::MergeWakeReq {
                work_id: WorkId::from("00000000-0000-4000-8000-000000000011"),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "scheduler".to_owned(),
                obligations: vec!["follow up".to_owned()],
            })
            .expect("wake should create queued run");
        let run_id = store
            .load_runtime_turn(&crate::model::RunId::from("run-2"))
            .expect("queued run should exist")
            .run_id;

        let ack = handle_resume_session(
            &store,
            &runtime,
            ResumeSessionCmd {
                run_id: run_id.clone(),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("queued run should start runtime");

        let saved = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from("00000000-0000-4000-8000-000000000011"),
            })
            .expect("session load should work")
            .expect("session should be saved");

        assert!(!ack.resumed);
        assert_eq!(ack.runtime_session_id, "runtime-queued");
        assert_eq!(saved.runtime_session_id, "runtime-queued");
        assert_eq!(saved.runtime, RuntimeKind::Coclai);
        assert_eq!(store.read_board().consumption_summary.total_turns, 1);
        assert_eq!(
            store.read_board().consumption_summary.total_input_tokens,
            120
        );
        assert_eq!(
            store
                .read_agents()
                .consumption_by_agent
                .iter()
                .find(|summary| summary.agent_id == DEMO_AGENT_ID)
                .expect("agent rollup should exist")
                .total_estimated_cost_cents,
            7
        );
        assert!(store
            .read_activity()
            .entries
            .iter()
            .any(|entry| { entry.event_kind == "run" && entry.summary.contains("run-2 running") }));
    }

    #[test]
    fn resume_session_prefers_existing_matching_session() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-existing".to_owned(),
                },
                raw_output: valid_output("complete"),
                intent: intent(TransitionKind::Complete),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-existing"),
                company_id: CompanyId::from("00000000-0000-4000-8000-000000000001"),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-existing".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: Some("accepted earlier".to_owned()),
                last_gate_summary: Some("gate ok".to_owned()),
                updated_at: SystemTime::UNIX_EPOCH,
            })
            .expect("session seed should save");

        let ack = handle_resume_session(
            &store,
            &runtime,
            ResumeSessionCmd {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("matching session should resume");

        assert!(ack.resumed);
        assert_eq!(ack.intent_kind, TransitionKind::Complete);
        assert_eq!(ack.session_reset_reason, None);
    }

    #[test]
    fn resume_session_resets_when_workspace_changes() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-new-workspace".to_owned(),
                },
                raw_output: valid_output("complete"),
                intent: intent(TransitionKind::Complete),
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        );
        let store = MemoryStore::demo();
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-existing"),
                company_id: CompanyId::from("00000000-0000-4000-8000-000000000001"),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-existing".to_owned(),
                cwd: "/old-repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/old-repo"),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: Some("accepted earlier".to_owned()),
                last_gate_summary: Some("gate ok".to_owned()),
                updated_at: SystemTime::UNIX_EPOCH,
            })
            .expect("session seed should save");

        let ack = handle_resume_session(
            &store,
            &runtime,
            ResumeSessionCmd {
                run_id: crate::model::RunId::from("run-1"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect("workspace mismatch should reset session");

        let saved = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
            })
            .expect("session lookup should work")
            .expect("session should be saved");

        assert!(!ack.resumed);
        assert_eq!(
            ack.session_reset_reason,
            Some(SessionInvalidationReason::Workspace)
        );
        assert_eq!(saved.session_id, SessionId::from("session-run-1"));
        assert_eq!(saved.runtime_session_id, "runtime-new-workspace");
    }

    #[test]
    fn resume_session_marks_run_failed_with_runtime_failure_activity() {
        let assets = load_runtime_assets();
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                ScriptedReply {
                    handle: RuntimeHandle {
                        runtime_session_id: "runtime-bad".to_owned(),
                    },
                    raw_output: "{\"kind\":\"complete\"}".to_owned(),
                    intent: intent(TransitionKind::Complete),
                    usage: sample_usage(120, 48, 3, 7),
                    invalid_session: false,
                },
                ScriptedReply {
                    handle: RuntimeHandle {
                        runtime_session_id: "runtime-bad".to_owned(),
                    },
                    raw_output: "{\"kind\":\"complete\"}".to_owned(),
                    intent: intent(TransitionKind::Complete),
                    usage: sample_usage(120, 48, 3, 7),
                    invalid_session: false,
                },
            ],
        );
        let store = MemoryStore::demo();
        store
            .merge_wake(crate::port::store::MergeWakeReq {
                work_id: WorkId::from("00000000-0000-4000-8000-000000000011"),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "scheduler".to_owned(),
                obligations: vec!["follow up".to_owned()],
            })
            .expect("wake should create queued run");

        let error = handle_resume_session(
            &store,
            &runtime,
            ResumeSessionCmd {
                run_id: RunId::from("run-2"),
                cwd: "/repo".to_owned(),
            },
        )
        .expect_err("runtime failure should bubble up");

        let run = store
            .read_run(&RunId::from("run-2"))
            .expect("run read should succeed");
        let activity = store.read_activity();

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Unavailable);
        assert_eq!(run.status, "failed");
        assert!(activity.entries.iter().any(|entry| {
            entry.event_kind == "run"
                && entry.source.as_deref() == Some("runtime")
                && entry.outcome.as_deref() == Some("failed")
                && entry.summary.contains("runtime_failure")
        }));
        assert!(!activity.entries.iter().any(|entry| {
            entry.event_kind == "transition"
                && entry.outcome.as_deref() == Some("rejected")
                && entry.summary.contains("runtime_failure")
        }));
    }

    fn intent(kind: TransitionKind) -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from("00000000-0000-4000-8000-000000000013"),
            expected_rev: 1,
            kind,
            patch: WorkPatch {
                summary: "runtime turn".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: Vec::new(),
        }
    }

    fn valid_output(kind: &str) -> String {
        runtime_intent_output(RuntimeIntentOutput {
            work_id: DEMO_DOING_WORK_ID,
            agent_id: DEMO_AGENT_ID,
            lease_id: "00000000-0000-4000-8000-000000000013",
            expected_rev: 1,
            kind,
            summary: "runtime turn",
            note: None,
            proof_hints: &[],
        })
    }
}
