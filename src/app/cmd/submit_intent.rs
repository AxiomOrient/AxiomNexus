use serde::Serialize;

use std::time::Duration;

use crate::{
    kernel,
    model::{
        ActorId, ActorKind, DecisionOutcome, EvidenceBundle, EvidenceInline, GateSpec,
        ProofHintKind, RecordId, TaskSession, TransitionIntent, TransitionRecord,
    },
    port::{
        store::{CommandStorePort, CommitDecisionReq, SessionKey, StoreError, StoreErrorKind},
        workspace::WorkspacePort,
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
    workspace: &impl WorkspacePort,
    cmd: SubmitIntentCmd,
) -> Result<SubmitIntentAck, StoreError> {
    let context = store.load_context(&cmd.intent.work_id)?;
    let evidence = collect_decision_evidence(store, workspace, &context, &cmd.intent)?;
    let decision = kernel::decide_transition(
        &context.snapshot,
        context.lease.as_ref(),
        context.pending_wake.as_ref(),
        &context.contract,
        &evidence,
        &cmd.intent,
    );
    let record = record_from_decision(&context, &cmd.intent, &decision);
    let session = session_from_decision(store, &cmd.intent, &context.contract, &record, &decision)?;
    let committed = store.commit_decision(CommitDecisionReq {
        decision: decision.clone(),
        record,
        session,
    })?;

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

fn collect_decision_evidence(
    store: &impl CommandStorePort,
    workspace: &impl WorkspacePort,
    context: &crate::port::store::WorkContext,
    intent: &TransitionIntent,
) -> Result<EvidenceBundle, StoreError> {
    let cwd = gate_command_cwd(store, workspace, intent)?;
    let (observed_agent_status, observed_agent_company_id) = observed_agent_facts(store, intent)?;
    let hinted_paths = intent
        .proof_hints
        .iter()
        .filter(|hint| hint.kind == ProofHintKind::File)
        .map(|hint| hint.value.clone())
        .collect::<Vec<_>>();
    let mut evidence = EvidenceBundle {
        changed_files: workspace.observe_changed_files(&cwd, &hinted_paths),
        observed_agent_status,
        observed_agent_company_id,
        ..EvidenceBundle::default()
    };
    let command_gates = kernel::command_gate_specs(
        &context.snapshot,
        context.lease.as_ref(),
        &context.contract,
        intent,
    );

    if command_gates.is_empty() {
        return Ok(evidence);
    }

    for gate in command_gates {
        let GateSpec::CommandSucceeds {
            argv, timeout_sec, ..
        } = gate
        else {
            continue;
        };

        evidence.command_results.push(workspace.run_gate_command(
            &cwd,
            &argv,
            Duration::from_secs(timeout_sec),
        ));
    }

    Ok(evidence)
}

fn observed_agent_facts(
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

fn gate_command_cwd(
    store: &impl CommandStorePort,
    workspace: &impl WorkspacePort,
    intent: &TransitionIntent,
) -> Result<String, StoreError> {
    let key = SessionKey {
        agent_id: intent.agent_id.clone(),
        work_id: intent.work_id.clone(),
    };
    if let Some(session) = store.load_session(&key)? {
        return Ok(session.cwd);
    }

    workspace.current_dir().map_err(|error| StoreError {
        kind: StoreErrorKind::Unavailable,
        message: format!("submit_intent could not resolve current cwd: {error}"),
    })
}

fn record_from_decision(
    context: &crate::port::store::WorkContext,
    intent: &TransitionIntent,
    decision: &crate::model::TransitionDecision,
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
        lease_id: context.lease.as_ref().map(|lease| lease.lease_id.clone()),
        expected_rev: intent.expected_rev,
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
    store: &impl CommandStorePort,
    intent: &TransitionIntent,
    contract: &crate::model::ContractSet,
    record: &TransitionRecord,
    decision: &crate::model::TransitionDecision,
) -> Result<Option<TaskSession>, StoreError> {
    let key = SessionKey {
        agent_id: intent.agent_id.clone(),
        work_id: intent.work_id.clone(),
    };
    let existing = store.load_session(&key)?;
    let Some(existing) = existing else {
        return Ok(None);
    };

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

    Ok(Some(kernel::advance_session(
        Some(&existing),
        candidate,
        false,
    )))
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
        adapter::memory::store::{
            MemoryStore, DEMO_AGENT_ID, DEMO_COMPANY_ID, DEMO_CONTRACT_SET_ID, DEMO_DOING_WORK_ID,
            DEMO_LEASE_ID,
        },
        model::{
            AgentId, ChangeKind, CommandResult, CompanyId, ContractSet, ContractSetId,
            ContractSetStatus, FileChange, GateSpec, LeaseId, Priority, RuntimeKind, SessionId,
            TaskSession, TransitionIntent, TransitionKind, TransitionRule, WorkId, WorkKind,
            WorkLease, WorkPatch, WorkSnapshot, WorkStatus,
        },
        port::{
            store::{SessionKey, StorePort, WorkContext},
            workspace::{WorkspaceError, WorkspacePort},
        },
    };

    use super::{collect_decision_evidence, handle_submit_intent, SubmitIntentCmd};

    #[test]
    fn accepted_submit_intent_updates_session_decision_summary() {
        let store = MemoryStore::demo();
        seed_session(&store, Some("old accepted"), Some("old gate"));

        let ack = handle_submit_intent(
            &store,
            &TestWorkspace::default(),
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
    fn rejected_submit_intent_updates_session_gate_summary() {
        let store = MemoryStore::demo();
        seed_session(&store, Some("accepted earlier"), None);

        let ack = handle_submit_intent(
            &store,
            &TestWorkspace::default(),
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
    fn collect_decision_evidence_executes_allowed_command_gate_and_tracks_changed_files() {
        let store = MemoryStore::demo();
        seed_session_at_cwd(&store, Some("accepted earlier"), None, "/repo");
        let intent = complete_intent_with_file_hint("tracked.txt");
        let context = complete_context(GateSpec::CommandSucceeds {
            argv: vec!["cargo".to_owned(), "--version".to_owned()],
            timeout_sec: 30,
            allow_exit_codes: vec![0],
        });
        let workspace = TestWorkspace {
            changed_files: vec![FileChange {
                path: "tracked.txt".to_owned(),
                change_kind: ChangeKind::Modified,
            }],
            command_result: CommandResult {
                argv: vec!["cargo".to_owned(), "--version".to_owned()],
                exit_code: 0,
                stdout: "cargo 1.0.0".to_owned(),
                stderr: String::new(),
                failure_detail: None,
            },
            ..TestWorkspace::default()
        };

        let evidence = collect_decision_evidence(&store, &workspace, &context, &intent)
            .expect("evidence should collect");

        assert_eq!(evidence.changed_files.len(), 1);
        assert_eq!(evidence.changed_files[0].path, "tracked.txt");
        assert_eq!(evidence.command_results.len(), 1);
        assert_eq!(evidence.command_results[0].exit_code, 0);
        assert!(evidence.command_results[0].failure_detail.is_none());
        assert!(evidence.command_results[0].stdout.contains("cargo"));
    }

    #[test]
    fn collect_decision_evidence_marks_disallowed_command_gate_failure() {
        let store = MemoryStore::demo();
        seed_session_at_cwd(&store, Some("accepted earlier"), None, "/repo");
        let intent = complete_intent_with_file_hint("tracked.txt");
        let context = complete_context(GateSpec::CommandSucceeds {
            argv: vec!["echo".to_owned(), "nope".to_owned()],
            timeout_sec: 30,
            allow_exit_codes: vec![0],
        });
        let workspace = TestWorkspace {
            changed_files: vec![FileChange {
                path: "tracked.txt".to_owned(),
                change_kind: ChangeKind::Modified,
            }],
            command_result: CommandResult {
                argv: vec!["echo".to_owned(), "nope".to_owned()],
                exit_code: -1,
                stdout: String::new(),
                stderr: String::new(),
                failure_detail: Some("command argv is not in the allowlist".to_owned()),
            },
            ..TestWorkspace::default()
        };

        let evidence = collect_decision_evidence(&store, &workspace, &context, &intent)
            .expect("evidence should collect");

        assert_eq!(evidence.command_results.len(), 1);
        assert_eq!(evidence.command_results[0].exit_code, -1);
        assert!(evidence.command_results[0]
            .failure_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("allowlist")));
    }

    #[test]
    fn collect_decision_evidence_ignores_unobserved_file_hints() {
        let store = MemoryStore::demo();
        seed_session_at_cwd(&store, Some("accepted earlier"), None, "/repo");
        let intent = complete_intent_with_file_hint("not-changed.txt");
        let context = complete_context(GateSpec::ChangedFilesObserved);

        let evidence = collect_decision_evidence(
            &store,
            &TestWorkspace {
                changed_files: vec![FileChange {
                    path: "tracked.txt".to_owned(),
                    change_kind: ChangeKind::Modified,
                }],
                ..TestWorkspace::default()
            },
            &context,
            &intent,
        )
        .expect("evidence should collect");

        assert!(evidence.changed_files.is_empty());
    }

    #[derive(Debug, Clone)]
    struct TestWorkspace {
        current_dir: String,
        changed_files: Vec<FileChange>,
        command_result: CommandResult,
    }

    impl Default for TestWorkspace {
        fn default() -> Self {
            Self {
                current_dir: String::new(),
                changed_files: Vec::new(),
                command_result: CommandResult {
                    argv: Vec::new(),
                    exit_code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                    failure_detail: None,
                },
            }
        }
    }

    impl WorkspacePort for TestWorkspace {
        fn current_dir(&self) -> Result<String, WorkspaceError> {
            if self.current_dir.is_empty() {
                return Ok("/repo".to_owned());
            }

            Ok(self.current_dir.clone())
        }

        fn observe_changed_files(&self, _cwd: &str, hinted_paths: &[String]) -> Vec<FileChange> {
            self.changed_files
                .iter()
                .filter(|change| hinted_paths.iter().any(|path| path == &change.path))
                .cloned()
                .collect()
        }

        fn run_gate_command(
            &self,
            _cwd: &str,
            _argv: &[String],
            _timeout: std::time::Duration,
        ) -> CommandResult {
            self.command_result.clone()
        }
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
}
