use serde::Serialize;

use std::time::Duration;

use crate::{
    kernel,
    model::{
        ActorId, ActorKind, DecisionOutcome, EvidenceBundle, EvidenceInline, RecordId, TaskSession,
        TransitionIntent, TransitionRecord,
    },
    port::{
        runtime::RuntimeObservations,
        store::{CommandStorePort, CommitDecisionReq, SessionKey, StoreError},
    },
};

use super::DECISION_PATH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubmitIntentCmd {
    pub(crate) intent: TransitionIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SubmitIntentAck {
    pub(crate) decision_path: &'static str,
    pub(crate) outcome: DecisionOutcome,
    pub(crate) summary: String,
    #[serde(skip_serializing)]
    pub(crate) after_commit_event_data: String,
}

pub(crate) fn handle_submit_intent(
    store: &impl CommandStorePort,
    cmd: SubmitIntentCmd,
) -> Result<SubmitIntentAck, StoreError> {
    let context = store.load_context(&cmd.intent.work_id)?;
    let evidence = collect_direct_submit_evidence(store, &cmd.intent)?;
    commit_runtime_intent(store, &context, &cmd.intent, evidence)
}

pub(crate) fn collect_direct_submit_evidence(
    store: &impl CommandStorePort,
    intent: &TransitionIntent,
) -> Result<EvidenceBundle, StoreError> {
    let (observed_agent_status, observed_agent_company_id) = observed_agent_facts(store, intent)?;
    Ok(EvidenceBundle {
        observed_agent_status,
        observed_agent_company_id,
        ..EvidenceBundle::default()
    })
}

pub(crate) fn collect_runtime_observation_evidence(
    store: &impl CommandStorePort,
    observations: &RuntimeObservations,
    intent: &TransitionIntent,
) -> Result<EvidenceBundle, StoreError> {
    let (observed_agent_status, observed_agent_company_id) = observed_agent_facts(store, intent)?;
    Ok(EvidenceBundle {
        changed_files: observations.changed_files.clone(),
        command_results: observations.command_results.clone(),
        artifact_refs: observations.artifact_refs.clone(),
        observed_agent_status,
        observed_agent_company_id,
        ..EvidenceBundle::default()
    })
}

pub(crate) fn commit_runtime_intent(
    store: &impl CommandStorePort,
    context: &crate::port::store::WorkContext,
    intent: &TransitionIntent,
    evidence: EvidenceBundle,
) -> Result<SubmitIntentAck, StoreError> {
    let session_key = SessionKey {
        agent_id: intent.agent_id.clone(),
        work_id: intent.work_id.clone(),
    };
    let existing_session = store.load_session(&session_key)?;
    let decision = kernel::decide_transition(
        &context.snapshot,
        context.lease.as_ref(),
        context.pending_wake.as_ref(),
        &context.contract,
        &evidence,
        intent,
    );
    let record = record_from_decision(context, intent, &decision, existing_session.as_ref());
    let session = session_from_decision(existing_session, &context.contract, &record, &decision);
    let committed =
        store.commit_decision(CommitDecisionReq::new(decision.clone(), record, session))?;

    Ok(SubmitIntentAck {
        decision_path: DECISION_PATH,
        outcome: decision.outcome,
        summary: decision.summary,
        after_commit_event_data: serde_json::to_string(
            &committed
                .activity_event
                .expect("commit_decision should persist an activity event"),
        )
        .expect("activity event json should serialize"),
    })
}

pub(crate) fn observed_agent_facts(
    store: &impl CommandStorePort,
    intent: &TransitionIntent,
) -> Result<
    (
        Option<crate::model::AgentStatus>,
        Option<crate::model::CompanyId>,
    ),
    StoreError,
> {
    let agent = store.load_agent_facts(&intent.agent_id)?;
    Ok((
        agent.as_ref().map(|agent| agent.status),
        agent.map(|agent| agent.company_id),
    ))
}

fn record_from_decision(
    context: &crate::port::store::WorkContext,
    intent: &TransitionIntent,
    decision: &crate::model::TransitionDecision,
    existing_session: Option<&TaskSession>,
) -> TransitionRecord {
    TransitionRecord {
        record_id: RecordId::from(format!(
            "record-{}-{}-{:?}",
            context.snapshot.work_id,
            context.snapshot.rev + 1,
            intent.kind
        )),
        company_id: context.snapshot.company_id.clone(),
        work_id: intent.work_id.clone(),
        actor_kind: actor_kind_for(intent.kind),
        actor_id: actor_id_for(intent.kind, intent),
        run_id: context
            .lease
            .as_ref()
            .and_then(|lease| lease.run_id.clone()),
        session_id: existing_session.map(|session| session.session_id.clone()),
        lease_id: context.lease.as_ref().map(|lease| lease.lease_id.clone()),
        expected_rev: intent.expected_rev,
        contract_set_id: context.snapshot.contract_set_id.clone(),
        contract_rev: context.snapshot.contract_rev,
        before_status: context.snapshot.status,
        after_status: decision
            .next_snapshot
            .as_ref()
            .map(|snapshot| snapshot.status),
        outcome: decision.outcome,
        reasons: decision.reasons.clone(),
        kind: intent.kind,
        patch: intent.patch.clone(),
        gate_results: decision.gate_results.clone(),
        evidence: decision.evidence.clone(),
        evidence_inline: Some(EvidenceInline {
            summary: decision.summary.clone(),
        }),
        evidence_refs: decision.evidence.artifact_refs.clone(),
        happened_at: context.snapshot.updated_at + Duration::from_secs(1),
    }
}

fn session_from_decision(
    existing: Option<TaskSession>,
    contract: &crate::model::ContractSet,
    record: &TransitionRecord,
    decision: &crate::model::TransitionDecision,
) -> Option<TaskSession> {
    let existing = existing?;

    let mut candidate = existing.clone();
    candidate.contract_rev = contract.revision;
    candidate.last_record_id = Some(record.record_id.clone());
    candidate.updated_at = record.happened_at;

    match decision.outcome {
        DecisionOutcome::Accepted | DecisionOutcome::OverrideAccepted => {
            candidate.last_decision_summary = Some(decision.summary.clone());
            candidate.last_gate_summary = None;
        }
        DecisionOutcome::Rejected | DecisionOutcome::Conflict => {
            candidate.last_gate_summary = Some(decision.summary.clone());
        }
    }

    Some(kernel::advance_session(Some(&existing), candidate, None))
}

fn actor_kind_for(kind: crate::model::TransitionKind) -> ActorKind {
    match kind {
        crate::model::TransitionKind::Queue
        | crate::model::TransitionKind::Reopen
        | crate::model::TransitionKind::Cancel
        | crate::model::TransitionKind::OverrideComplete => ActorKind::Board,
        crate::model::TransitionKind::TimeoutRequeue => ActorKind::System,
        crate::model::TransitionKind::Claim
        | crate::model::TransitionKind::ProposeProgress
        | crate::model::TransitionKind::Complete
        | crate::model::TransitionKind::Block => ActorKind::Agent,
    }
}

fn actor_id_for(kind: crate::model::TransitionKind, intent: &TransitionIntent) -> ActorId {
    match actor_kind_for(kind) {
        ActorKind::Agent => ActorId::from(intent.agent_id.as_str()),
        ActorKind::Board => ActorId::from("board"),
        ActorKind::System => ActorId::from("system"),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::{
            coclai::runtime::{CoclaiRuntime, ScriptedReply},
            memory::store::{
                MemoryStore, DEMO_AGENT_ID, DEMO_COMPANY_ID, DEMO_CONTRACT_SET_ID,
                DEMO_DOING_WORK_ID, DEMO_LEASE_ID,
            },
            surreal::store::SurrealStore,
        },
        app::cmd::{
            activate_contract::{handle_activate_contract, ActivateContractCmd},
            claim_work::{handle_claim_work, ClaimWorkCmd},
            create_agent::{handle_create_agent, CreateAgentCmd},
            create_company::{handle_create_company, CreateCompanyCmd},
            create_contract_draft::{handle_create_contract_draft, CreateContractDraftCmd},
            create_work::{handle_create_work, CreateWorkCmd},
            resume_session::{handle_resume_session, ResumeSessionCmd},
            test_support::{
                load_runtime_assets, runtime_intent_output, sample_usage, RuntimeIntentOutput,
            },
        },
        model::{
            workspace_fingerprint, ActorKind, AgentId, CompanyId, ContractSet, ContractSetId,
            ContractSetStatus, DecisionOutcome, GateSpec, LeaseEffect, LeaseId, Priority,
            RuntimeKind, SessionId, TaskSession, TransitionIntent, TransitionKind, TransitionRule,
            WorkId, WorkKind, WorkLease, WorkPatch, WorkSnapshot, WorkStatus,
        },
        port::{
            runtime::RuntimeHandle,
            store::{SessionKey, StorePort, WorkContext},
        },
    };

    use super::{handle_submit_intent, SubmitIntentCmd};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FlowSummary {
        final_status: WorkStatus,
        final_rev: u64,
        replay_matches_live: bool,
        record_summaries: Vec<FlowRecordSummary>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FlowRecordSummary {
        kind: TransitionKind,
        outcome: DecisionOutcome,
        expected_rev: u64,
        before_status: WorkStatus,
        after_status: Option<WorkStatus>,
    }

    #[test]
    fn accepted_submit_intent_updates_session_decision_summary() {
        let store = MemoryStore::demo();
        seed_session(&store, Some("old accepted"), Some("old gate"));

        let ack = handle_submit_intent(
            &store,
            SubmitIntentCmd {
                intent: TransitionIntent {
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    agent_id: AgentId::from(DEMO_AGENT_ID),
                    lease_id: LeaseId::from(DEMO_LEASE_ID),
                    expected_rev: 1,
                    kind: TransitionKind::ProposeProgress,
                    patch: WorkPatch {
                        summary: "progress update".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    note: None,
                    proof_hints: vec![crate::model::ProofHint {
                        kind: crate::model::ProofHintKind::Summary,
                        value: "progress update".to_owned(),
                    }],
                },
            },
        )
        .expect("accepted submit should succeed");

        let session = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
            })
            .expect("session lookup should work")
            .expect("session should still exist");

        assert_eq!(ack.outcome, crate::model::DecisionOutcome::Accepted);
        assert_eq!(
            session.last_decision_summary.as_deref(),
            Some("ProposeProgress Accepted with next status Doing")
        );
        assert!(ack
            .after_commit_event_data
            .contains("\"summary\":\"ProposeProgress Accepted with next status Doing\""));
        assert!(session.last_gate_summary.is_none());
        assert!(session.last_record_id.is_some());
    }

    #[test]
    fn handle_submit_intent_stays_workspace_free_for_direct_path() {
        let store = MemoryStore::demo();
        seed_session(&store, Some("accepted earlier"), Some("old gate"));

        let ack = handle_submit_intent(
            &store,
            SubmitIntentCmd {
                intent: TransitionIntent {
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    agent_id: AgentId::from(DEMO_AGENT_ID),
                    lease_id: LeaseId::from(DEMO_LEASE_ID),
                    expected_rev: 1,
                    kind: TransitionKind::ProposeProgress,
                    patch: WorkPatch {
                        summary: "progress update".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    note: None,
                    proof_hints: vec![crate::model::ProofHint {
                        kind: crate::model::ProofHintKind::Summary,
                        value: "progress update".to_owned(),
                    }],
                },
            },
        )
        .expect("direct submit should no longer need workspace");

        assert_eq!(ack.outcome, crate::model::DecisionOutcome::Accepted);
    }

    #[test]
    fn rejected_submit_intent_updates_session_gate_summary() {
        let store = MemoryStore::demo();
        seed_session(&store, Some("accepted earlier"), None);

        let ack = handle_submit_intent(
            &store,
            SubmitIntentCmd {
                intent: TransitionIntent {
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    agent_id: AgentId::from(DEMO_AGENT_ID),
                    lease_id: LeaseId::from(DEMO_LEASE_ID),
                    expected_rev: 1,
                    kind: TransitionKind::Block,
                    patch: WorkPatch {
                        summary: "blocked".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: vec!["waiting".to_owned()],
                    },
                    note: None,
                    proof_hints: vec![crate::model::ProofHint {
                        kind: crate::model::ProofHintKind::Summary,
                        value: "blocked".to_owned(),
                    }],
                },
            },
        )
        .expect("rejected submit still commits record");

        let session = store
            .load_session(&SessionKey {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
            })
            .expect("session lookup should work")
            .expect("session should still exist");

        assert_eq!(ack.outcome, crate::model::DecisionOutcome::Rejected);
        assert_eq!(
            session.last_decision_summary.as_deref(),
            Some("accepted earlier")
        );
        assert_eq!(
            session.last_gate_summary.as_deref(),
            Some("manual transition requires note")
        );
        assert!(ack
            .after_commit_event_data
            .contains("\"summary\":\"manual transition requires note\""));
        assert!(session.last_record_id.is_some());
    }

    #[test]
    fn memory_queue_turn_commit_replay_flow_stays_consistent() {
        let store = MemoryStore::demo();
        let created = handle_create_work(
            &store,
            CreateWorkCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Memory replay flow".to_owned(),
                body: "Exercise queue -> turn -> commit -> replay".to_owned(),
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            },
        )
        .expect("work should create");
        let work_id = WorkId::from(created.work_id.as_str());
        let summary = execute_runtime_flow(
            &store,
            &work_id,
            &AgentId::from(DEMO_AGENT_ID),
            "queued for runtime",
        );

        assert!(summary.replay_matches_live);
        assert_eq!(summary.record_summaries.len(), 3);
        assert_eq!(summary.record_summaries[0].kind, TransitionKind::Queue);
        assert_eq!(summary.record_summaries[1].kind, TransitionKind::Claim);
        assert_eq!(
            summary.record_summaries[2].kind,
            TransitionKind::ProposeProgress
        );
    }

    #[test]
    fn surreal_queue_turn_commit_replay_flow_stays_consistent() {
        let (store, work_id, agent_id) =
            setup_surreal_runtime_flow_fixture("submit-intent-live-flow");
        let summary = execute_runtime_flow(&store, &work_id, &agent_id, "queued for live runtime");

        assert!(summary.replay_matches_live);
        assert_eq!(summary.record_summaries.len(), 3);
        assert_eq!(summary.record_summaries[0].kind, TransitionKind::Queue);
        assert_eq!(summary.record_summaries[1].kind, TransitionKind::Claim);
        assert_eq!(
            summary.record_summaries[2].kind,
            TransitionKind::ProposeProgress
        );
    }

    #[test]
    fn runtime_flow_parity_matches_between_memory_and_surreal() {
        let memory_store = MemoryStore::demo();
        let memory_created = handle_create_work(
            &memory_store,
            CreateWorkCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Memory parity flow".to_owned(),
                body: "Compare runtime parity".to_owned(),
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            },
        )
        .expect("memory work should create");
        let memory_summary = execute_runtime_flow(
            &memory_store,
            &WorkId::from(memory_created.work_id.as_str()),
            &AgentId::from(DEMO_AGENT_ID),
            "queued for parity",
        );

        let (surreal_store, surreal_work_id, surreal_agent_id) =
            setup_surreal_runtime_flow_fixture("submit-intent-parity-flow");
        let surreal_summary = execute_runtime_flow(
            &surreal_store,
            &surreal_work_id,
            &surreal_agent_id,
            "queued for parity",
        );

        assert_eq!(memory_summary, surreal_summary);
    }

    fn seed_session(store: &MemoryStore, decision: Option<&str>, gate: Option<&str>) {
        seed_session_at_cwd(store, decision, gate, "/repo");
    }

    fn seed_session_at_cwd(
        store: &MemoryStore,
        decision: Option<&str>,
        gate: Option<&str>,
        cwd: &str,
    ) {
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-1"),
                company_id: CompanyId::from("00000000-0000-4000-8000-000000000001"),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-1".to_owned(),
                cwd: cwd.to_owned(),
                workspace_fingerprint: workspace_fingerprint(cwd),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: decision.map(str::to_owned),
                last_gate_summary: gate.map(str::to_owned),
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            })
            .expect("seed session should save");
    }

    fn complete_intent_with_file_hint(path: &str) -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from(DEMO_LEASE_ID),
            expected_rev: 1,
            kind: TransitionKind::Complete,
            patch: WorkPatch {
                summary: "completed with checks".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: vec![
                crate::model::ProofHint {
                    kind: crate::model::ProofHintKind::Summary,
                    value: "completed with checks".to_owned(),
                },
                crate::model::ProofHint {
                    kind: crate::model::ProofHintKind::File,
                    value: path.to_owned(),
                },
            ],
        }
    }

    fn complete_context(command_gate: GateSpec) -> WorkContext {
        WorkContext {
            snapshot: WorkSnapshot {
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Doing work".to_owned(),
                body: String::new(),
                status: WorkStatus::Doing,
                priority: Priority::High,
                assignee_agent_id: Some(AgentId::from(DEMO_AGENT_ID)),
                active_lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
                rev: 1,
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                contract_rev: 1,
                created_at: std::time::SystemTime::UNIX_EPOCH,
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            },
            lease: Some(WorkLease {
                lease_id: LeaseId::from(DEMO_LEASE_ID),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                run_id: None,
                acquired_at: std::time::SystemTime::UNIX_EPOCH,
                expires_at: None,
                released_at: None,
                release_reason: None,
            }),
            pending_wake: None,
            contract: ContractSet {
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                revision: 1,
                name: "axiomnexus-rust-default".to_owned(),
                status: ContractSetStatus::Active,
                rules: vec![TransitionRule {
                    kind: TransitionKind::Complete,
                    actor_kind: crate::model::ActorKind::Agent,
                    from: vec![WorkStatus::Doing],
                    to: WorkStatus::Done,
                    lease_effect: crate::model::LeaseEffect::Release,
                    gates: vec![
                        GateSpec::LeasePresent,
                        GateSpec::LeaseHeldByActor,
                        GateSpec::ExpectedRevMatchesSnapshot,
                        GateSpec::SummaryPresent,
                        GateSpec::ChangedFilesObserved,
                        command_gate,
                    ],
                }],
            },
        }
    }

    fn progress_context_without_workspace_gates() -> WorkContext {
        WorkContext {
            snapshot: WorkSnapshot {
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Doing work".to_owned(),
                body: String::new(),
                status: WorkStatus::Doing,
                priority: Priority::High,
                assignee_agent_id: Some(AgentId::from(DEMO_AGENT_ID)),
                active_lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
                rev: 1,
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                contract_rev: 1,
                created_at: std::time::SystemTime::UNIX_EPOCH,
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            },
            lease: Some(WorkLease {
                lease_id: LeaseId::from(DEMO_LEASE_ID),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                run_id: None,
                acquired_at: std::time::SystemTime::UNIX_EPOCH,
                expires_at: None,
                released_at: None,
                release_reason: None,
            }),
            pending_wake: None,
            contract: ContractSet {
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                revision: 1,
                name: "axiomnexus-rust-default".to_owned(),
                status: ContractSetStatus::Active,
                rules: vec![TransitionRule {
                    kind: TransitionKind::ProposeProgress,
                    actor_kind: crate::model::ActorKind::Agent,
                    from: vec![WorkStatus::Doing],
                    to: WorkStatus::Doing,
                    lease_effect: crate::model::LeaseEffect::Keep,
                    gates: vec![
                        GateSpec::LeasePresent,
                        GateSpec::LeaseHeldByActor,
                        GateSpec::ExpectedRevMatchesSnapshot,
                        GateSpec::SummaryPresent,
                    ],
                }],
            },
        }
    }

    fn execute_runtime_flow(
        store: &impl StorePort,
        work_id: &WorkId,
        agent_id: &AgentId,
        queue_summary: &str,
    ) -> FlowSummary {
        let queued = handle_submit_intent(
            store,
            SubmitIntentCmd {
                intent: TransitionIntent {
                    work_id: work_id.clone(),
                    agent_id: agent_id.clone(),
                    lease_id: LeaseId::from("lease-unused"),
                    expected_rev: 0,
                    kind: TransitionKind::Queue,
                    patch: WorkPatch {
                        summary: queue_summary.to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    note: None,
                    proof_hints: vec![crate::model::ProofHint {
                        kind: crate::model::ProofHintKind::Summary,
                        value: queue_summary.to_owned(),
                    }],
                },
            },
        )
        .expect("queue should commit");
        assert_eq!(queued.outcome, DecisionOutcome::Accepted);

        let claimed = handle_claim_work(
            store,
            ClaimWorkCmd {
                work_id: work_id.clone(),
                agent_id: agent_id.clone(),
            },
        )
        .expect("claim should succeed");
        assert_eq!(claimed.snapshot_status, WorkStatus::Doing);

        let context = store.load_context(work_id).expect("context should load");
        let lease = context.lease.expect("claimed work should expose lease");
        let run_id = lease.run_id.clone().expect("claim should attach run");
        let runtime = scripted_runtime_reply(work_id, agent_id, context.snapshot.rev);

        let resumed = handle_resume_session(
            store,
            &runtime,
            ResumeSessionCmd {
                run_id,
                cwd: "/repo".to_owned(),
            },
        )
        .expect("turn should save session");
        assert_eq!(
            resumed.runtime_policy,
            crate::app::cmd::RUNTIME_RESUME_POLICY
        );
        assert!(!resumed.resumed);

        let committed = handle_submit_intent(
            store,
            SubmitIntentCmd {
                intent: runtime_intent(work_id, agent_id, &lease.lease_id, context.snapshot.rev),
            },
        )
        .expect("runtime submit should commit");
        assert_eq!(committed.outcome, DecisionOutcome::Accepted);

        let live = store
            .load_context(work_id)
            .expect("live context should load after commit")
            .snapshot;
        let records = store
            .load_transition_records(work_id)
            .expect("transition records should load");
        let replayed = crate::kernel::replay_snapshot_from_records(
            &crate::kernel::replay_base_snapshot(&live),
            &records,
        )
        .expect("replay should succeed");

        FlowSummary {
            final_status: live.status,
            final_rev: live.rev,
            replay_matches_live: replayed == live,
            record_summaries: records
                .into_iter()
                .map(|record| FlowRecordSummary {
                    kind: record.kind,
                    outcome: record.outcome,
                    expected_rev: record.expected_rev,
                    before_status: record.before_status,
                    after_status: record.after_status,
                })
                .collect(),
        }
    }

    fn runtime_intent(
        work_id: &WorkId,
        agent_id: &AgentId,
        lease_id: &LeaseId,
        expected_rev: u64,
    ) -> TransitionIntent {
        TransitionIntent {
            work_id: work_id.clone(),
            agent_id: agent_id.clone(),
            lease_id: lease_id.clone(),
            expected_rev,
            kind: TransitionKind::ProposeProgress,
            patch: WorkPatch {
                summary: "runtime progress".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note: None,
            proof_hints: vec![crate::model::ProofHint {
                kind: crate::model::ProofHintKind::Summary,
                value: "runtime progress".to_owned(),
            }],
        }
    }

    fn scripted_runtime_reply(
        work_id: &WorkId,
        agent_id: &AgentId,
        expected_rev: u64,
    ) -> CoclaiRuntime {
        let assets = load_runtime_assets();
        let runtime_lease_id = LeaseId::from("33333333-3333-4333-8333-333333333333");
        let intent = runtime_intent(work_id, agent_id, &runtime_lease_id, expected_rev);
        let raw_output = runtime_intent_output(RuntimeIntentOutput {
            work_id: work_id.as_str(),
            agent_id: agent_id.as_str(),
            lease_id: runtime_lease_id.as_str(),
            expected_rev,
            kind: "propose_progress",
            summary: "runtime progress",
            note: None,
            proof_hints: &[("summary", "runtime progress")],
        });

        CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: RuntimeHandle {
                    runtime_session_id: "runtime-memory-flow".to_owned(),
                },
                raw_output,
                intent,
                usage: sample_usage(120, 48, 3, 7),
                invalid_session: false,
            }],
        )
    }

    fn runtime_flow_rules() -> Vec<TransitionRule> {
        vec![
            TransitionRule {
                kind: TransitionKind::Queue,
                actor_kind: ActorKind::Board,
                from: vec![WorkStatus::Backlog],
                to: WorkStatus::Todo,
                lease_effect: LeaseEffect::None,
                gates: Vec::new(),
            },
            TransitionRule {
                kind: TransitionKind::Claim,
                actor_kind: ActorKind::Agent,
                from: vec![WorkStatus::Todo],
                to: WorkStatus::Doing,
                lease_effect: LeaseEffect::Acquire,
                gates: vec![GateSpec::NoOpenLease, GateSpec::AgentIsRunnable],
            },
            TransitionRule {
                kind: TransitionKind::ProposeProgress,
                actor_kind: ActorKind::Agent,
                from: vec![WorkStatus::Doing],
                to: WorkStatus::Doing,
                lease_effect: LeaseEffect::Keep,
                gates: vec![
                    GateSpec::LeasePresent,
                    GateSpec::LeaseHeldByActor,
                    GateSpec::ExpectedRevMatchesSnapshot,
                    GateSpec::SummaryPresent,
                ],
            },
        ]
    }

    fn new_surreal_store(label: &str) -> SurrealStore {
        SurrealStore::open(&new_surreal_store_url(label)).expect("surreal store should open")
    }

    fn setup_surreal_runtime_flow_fixture(label: &str) -> (SurrealStore, WorkId, AgentId) {
        let store = new_surreal_store(label);
        let company = handle_create_company(
            &store,
            CreateCompanyCmd {
                name: "Surreal Co".to_owned(),
                description: "live runtime scenario".to_owned(),
                runtime_hard_stop_cents: None,
            },
        )
        .expect("company should create");
        let company_id = CompanyId::from(company.company_id.as_str());
        let agent = handle_create_agent(
            &store,
            CreateAgentCmd {
                company_id: company_id.clone(),
                name: "Live Agent".to_owned(),
                role: "builder".to_owned(),
            },
        )
        .expect("agent should create");
        let agent_id = AgentId::from(agent.agent_id.as_str());
        let draft = handle_create_contract_draft(
            &store,
            CreateContractDraftCmd {
                company_id: company_id.clone(),
                name: "live-runtime".to_owned(),
                rules: runtime_flow_rules(),
            },
        )
        .expect("draft should create");
        handle_activate_contract(
            &store,
            ActivateContractCmd {
                company_id: company_id.clone(),
                revision: draft.revision,
            },
        )
        .expect("contract should activate");

        let contract_set_id = ContractSetId::from(store.read_contracts().contract_set_id.as_str());
        let created = handle_create_work(
            &store,
            CreateWorkCmd {
                company_id,
                parent_id: None,
                kind: WorkKind::Task,
                title: "Surreal replay flow".to_owned(),
                body: "Exercise queue -> turn -> commit -> replay on live store".to_owned(),
                contract_set_id,
            },
        )
        .expect("work should create");

        (store, WorkId::from(created.work_id.as_str()), agent_id)
    }

    fn new_surreal_store_url(label: &str) -> String {
        format!(
            "surrealkv://{}/axiomnexus-submit-intent-{}-{}",
            std::env::temp_dir().display(),
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::from_secs(0))
                .as_nanos()
        )
    }
}
