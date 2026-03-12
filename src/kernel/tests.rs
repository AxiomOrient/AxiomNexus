use std::time::{Duration, SystemTime};

use crate::model::{
    ActorKind, AgentId, AgentStatus, ChangeKind, CommandResult, CompanyId, ContractSet,
    ContractSetId, ContractSetStatus, DecisionOutcome, EvidenceBundle, FileChange, GateSpec,
    LeaseEffect, LeaseId, PendingWake, PendingWakeEffect, ProofHint, ProofHintKind, ReasonCode,
    RecordId, Rev, RuntimeKind, SessionId, TaskSession, TransitionIntent, TransitionKind,
    TransitionRecord, TransitionRule, WorkId, WorkKind, WorkLease, WorkSnapshot, WorkStatus,
};

use super::{
    advance_session, claim_lease, command_gate_specs, decide_transition, merge_wake,
    replay_snapshot_from_records,
};

#[test]
fn claim_accepts_todo_and_acquires_lease() {
    let snapshot = snapshot(WorkStatus::Todo, None, 7);
    let intent = intent(TransitionKind::Claim, 7, "lease-1", None);
    let contract = contract(vec![rule(
        TransitionKind::Claim,
        ActorKind::Agent,
        vec![WorkStatus::Todo],
        WorkStatus::Doing,
        LeaseEffect::Acquire,
        vec![GateSpec::NoOpenLease, GateSpec::ExpectedRevMatchesSnapshot],
    )]);

    let decision = decide_transition(
        &snapshot,
        None,
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(decision.lease_effect, LeaseEffect::Acquire);
    assert_eq!(
        decision.next_snapshot.as_ref().map(|next| next.status),
        Some(WorkStatus::Doing)
    );
    assert_eq!(
        decision
            .next_snapshot
            .as_ref()
            .and_then(|next| next.active_lease_id.clone()),
        Some(LeaseId::from("lease-1"))
    );
}

#[test]
fn claim_conflicts_when_open_lease_exists() {
    let snapshot = snapshot(WorkStatus::Todo, Some("lease-open"), 3);
    let intent = intent(TransitionKind::Claim, 3, "lease-2", None);
    let contract = contract(vec![rule(
        TransitionKind::Claim,
        ActorKind::Agent,
        vec![WorkStatus::Todo],
        WorkStatus::Doing,
        LeaseEffect::Acquire,
        vec![GateSpec::NoOpenLease],
    )]);

    let decision = decide_transition(
        &snapshot,
        None,
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Conflict);
    assert_eq!(
        decision.reasons,
        vec![crate::model::ReasonCode::LeaseConflict]
    );
    assert!(decision.next_snapshot.is_none());
}

#[test]
fn claim_lease_accepts_work_without_open_lease() {
    let snapshot = snapshot(WorkStatus::Todo, None, 1);

    assert_eq!(claim_lease(&snapshot), Ok(()));
}

#[test]
fn claim_lease_rejects_work_with_open_lease() {
    let snapshot = snapshot(WorkStatus::Todo, Some("lease-open"), 1);

    assert_eq!(claim_lease(&snapshot), Err(ReasonCode::LeaseConflict));
}

#[test]
fn claim_rejects_when_agent_is_not_runnable() {
    let snapshot = snapshot(WorkStatus::Todo, None, 1);
    let intent = intent(TransitionKind::Claim, 1, "lease-1", None);
    let contract = contract(vec![rule(
        TransitionKind::Claim,
        ActorKind::Agent,
        vec![WorkStatus::Todo],
        WorkStatus::Doing,
        LeaseEffect::Acquire,
        vec![GateSpec::NoOpenLease, GateSpec::AgentIsRunnable],
    )]);
    let evidence = EvidenceBundle {
        observed_agent_status: Some(AgentStatus::Paused),
        observed_agent_company_id: Some(CompanyId::from("company-1")),
        ..EvidenceBundle::default()
    };

    let decision = decide_transition(&snapshot, None, None, &contract, &evidence, &intent);

    assert_eq!(decision.outcome, DecisionOutcome::Rejected);
    assert_eq!(decision.reasons, vec![ReasonCode::GateFailed]);
}

#[test]
fn claim_accepts_when_agent_is_active_in_same_company() {
    let snapshot = snapshot(WorkStatus::Todo, None, 1);
    let intent = intent(TransitionKind::Claim, 1, "lease-1", None);
    let contract = contract(vec![rule(
        TransitionKind::Claim,
        ActorKind::Agent,
        vec![WorkStatus::Todo],
        WorkStatus::Doing,
        LeaseEffect::Acquire,
        vec![GateSpec::NoOpenLease, GateSpec::AgentIsRunnable],
    )]);
    let evidence = EvidenceBundle {
        observed_agent_status: Some(AgentStatus::Active),
        observed_agent_company_id: Some(CompanyId::from("company-1")),
        ..EvidenceBundle::default()
    };

    let decision = decide_transition(&snapshot, None, None, &contract, &evidence, &intent);

    assert_eq!(decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(
        decision.next_snapshot.as_ref().map(|next| next.status),
        Some(WorkStatus::Doing)
    );
}

#[test]
fn propose_progress_keeps_status_and_lease() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-1"), 5);
    let lease = lease("lease-1", "agent-1");
    let intent = intent(TransitionKind::ProposeProgress, 5, "lease-1", None);
    let contract = contract(vec![rule(
        TransitionKind::ProposeProgress,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Doing,
        LeaseEffect::Keep,
        vec![
            GateSpec::LeasePresent,
            GateSpec::LeaseHeldByActor,
            GateSpec::SummaryPresent,
        ],
    )]);

    let decision = decide_transition(
        &snapshot,
        Some(&lease),
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(
        decision.next_snapshot.as_ref().map(|next| next.status),
        Some(WorkStatus::Doing)
    );
    assert_eq!(decision.lease_effect, LeaseEffect::Keep);
}

#[test]
fn complete_clears_pending_wake_when_obligations_are_resolved() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-1"), 9);
    let lease = lease("lease-1", "agent-1");
    let pending_wake = PendingWake {
        work_id: WorkId::from("work-1"),
        obligation_json: ["tests", "docs"].into_iter().map(str::to_owned).collect(),
        count: 2,
        latest_reason: "follow-up".to_owned(),
        merged_at: SystemTime::UNIX_EPOCH,
    };
    let mut intent = intent(TransitionKind::Complete, 9, "lease-1", None);
    intent.patch.resolved_obligations = vec!["tests".to_owned(), "docs".to_owned()];
    let contract = contract(vec![rule(
        TransitionKind::Complete,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Done,
        LeaseEffect::Release,
        vec![
            GateSpec::LeasePresent,
            GateSpec::LeaseHeldByActor,
            GateSpec::ChangedFilesObserved,
            GateSpec::AllRequiredObligationsResolved,
        ],
    )]);
    let evidence = EvidenceBundle {
        changed_files: vec![FileChange {
            path: "src/kernel/mod.rs".to_owned(),
            change_kind: ChangeKind::Modified,
        }],
        ..EvidenceBundle::default()
    };

    let decision = decide_transition(
        &snapshot,
        Some(&lease),
        Some(&pending_wake),
        &contract,
        &evidence,
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(decision.pending_wake_effect, PendingWakeEffect::Clear);
    assert_eq!(
        decision.next_snapshot.as_ref().map(|next| next.status),
        Some(WorkStatus::Done)
    );
    assert_eq!(
        decision
            .next_snapshot
            .as_ref()
            .and_then(|next| next.active_lease_id.clone()),
        None
    );
}

#[test]
fn block_requires_note() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-1"), 4);
    let lease = lease("lease-1", "agent-1");
    let intent = intent(TransitionKind::Block, 4, "lease-1", Some("   "));
    let contract = contract(vec![rule(
        TransitionKind::Block,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Blocked,
        LeaseEffect::Release,
        vec![
            GateSpec::LeasePresent,
            GateSpec::LeaseHeldByActor,
            GateSpec::ManualNotePresent,
        ],
    )]);

    let decision = decide_transition(
        &snapshot,
        Some(&lease),
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Rejected);
    assert_eq!(
        decision.reasons,
        vec![crate::model::ReasonCode::NoteMissing]
    );
}

#[test]
fn override_complete_is_board_only_and_returns_override_accepted() {
    let snapshot = snapshot(WorkStatus::Blocked, Some("lease-1"), 11);
    let intent = intent(
        TransitionKind::OverrideComplete,
        11,
        "lease-board",
        Some("manual override"),
    );
    let contract = contract(vec![rule(
        TransitionKind::OverrideComplete,
        ActorKind::Board,
        vec![WorkStatus::Doing, WorkStatus::Blocked],
        WorkStatus::Done,
        LeaseEffect::Release,
        vec![GateSpec::ManualNotePresent],
    )]);

    let decision = decide_transition(
        &snapshot,
        None,
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::OverrideAccepted);
    assert_eq!(
        decision.next_snapshot.as_ref().map(|next| next.status),
        Some(WorkStatus::Done)
    );
    assert_eq!(decision.lease_effect, LeaseEffect::Release);
}

#[test]
fn rev_mismatch_returns_conflict_without_mutating_snapshot() {
    let snapshot = snapshot(WorkStatus::Todo, None, 2);
    let intent = intent(TransitionKind::Claim, 1, "lease-1", None);
    let contract = contract(vec![rule(
        TransitionKind::Claim,
        ActorKind::Agent,
        vec![WorkStatus::Todo],
        WorkStatus::Doing,
        LeaseEffect::Acquire,
        vec![GateSpec::ExpectedRevMatchesSnapshot],
    )]);

    let decision = decide_transition(
        &snapshot,
        None,
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Conflict);
    assert_eq!(
        decision.reasons,
        vec![crate::model::ReasonCode::RevConflict]
    );
    assert!(decision.next_snapshot.is_none());
}

#[test]
fn stale_lease_rejects_runtime_intent() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-live"), 6);
    let lease = lease("lease-live", "agent-2");
    let intent = intent(TransitionKind::Complete, 6, "lease-live", None);
    let contract = contract(vec![rule(
        TransitionKind::Complete,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Done,
        LeaseEffect::Release,
        vec![GateSpec::LeasePresent, GateSpec::LeaseHeldByActor],
    )]);

    let decision = decide_transition(
        &snapshot,
        Some(&lease),
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Rejected);
    assert_eq!(decision.reasons, vec![crate::model::ReasonCode::StaleLease]);
}

#[test]
fn foreign_company_contract_is_rejected_even_when_id_and_revision_match() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-1"), 6);
    let lease = lease("lease-1", "agent-1");
    let intent = intent(TransitionKind::Complete, 6, "lease-1", None);
    let mut contract = contract(vec![rule(
        TransitionKind::Complete,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Done,
        LeaseEffect::Release,
        vec![GateSpec::LeasePresent, GateSpec::LeaseHeldByActor],
    )]);
    contract.company_id = CompanyId::from("company-2");

    let decision = decide_transition(
        &snapshot,
        Some(&lease),
        None,
        &contract,
        &EvidenceBundle::default(),
        &intent,
    );

    assert_eq!(decision.outcome, DecisionOutcome::Rejected);
    assert_eq!(decision.reasons, vec![ReasonCode::ContractDenied]);
}

#[test]
fn foreign_company_contract_disables_command_gate_collection() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-1"), 8);
    let lease = lease("lease-1", "agent-1");
    let intent = intent(TransitionKind::Complete, 8, "lease-1", None);
    let mut contract = contract(vec![rule(
        TransitionKind::Complete,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Done,
        LeaseEffect::Release,
        vec![
            GateSpec::LeasePresent,
            GateSpec::LeaseHeldByActor,
            GateSpec::CommandSucceeds {
                argv: vec!["cargo".to_owned(), "test".to_owned()],
                timeout_sec: 60,
                allow_exit_codes: vec![0],
            },
        ],
    )]);
    contract.company_id = CompanyId::from("company-2");

    let specs = command_gate_specs(&snapshot, Some(&lease), &contract, &intent);

    assert!(specs.is_empty());
}

#[test]
fn command_gate_reports_exit_code_and_failure_detail() {
    let snapshot = snapshot(WorkStatus::Doing, Some("lease-1"), 8);
    let lease = lease("lease-1", "agent-1");
    let intent = intent(TransitionKind::Complete, 8, "lease-1", None);
    let contract = contract(vec![rule(
        TransitionKind::Complete,
        ActorKind::Agent,
        vec![WorkStatus::Doing],
        WorkStatus::Done,
        LeaseEffect::Release,
        vec![
            GateSpec::LeasePresent,
            GateSpec::LeaseHeldByActor,
            GateSpec::CommandSucceeds {
                argv: vec!["cargo".to_owned(), "test".to_owned()],
                timeout_sec: 60,
                allow_exit_codes: vec![0],
            },
        ],
    )]);
    let evidence = EvidenceBundle {
        command_results: vec![CommandResult {
            argv: vec!["cargo".to_owned(), "test".to_owned()],
            exit_code: 101,
            stdout: String::new(),
            stderr: "failure".to_owned(),
            failure_detail: Some("command exited with code 101".to_owned()),
        }],
        ..EvidenceBundle::default()
    };

    let decision = decide_transition(&snapshot, Some(&lease), None, &contract, &evidence, &intent);

    assert_eq!(decision.outcome, DecisionOutcome::Rejected);
    assert_eq!(decision.reasons, vec![ReasonCode::GateFailed]);
    let command_gate = decision
        .gate_results
        .iter()
        .find(|result| matches!(result.gate, GateSpec::CommandSucceeds { .. }))
        .expect("command gate result should be present");
    assert!(command_gate.detail.contains("101"));
    assert!(command_gate.detail.contains("command exited with code 101"));
}

#[test]
fn merge_wake_coalesces_obligations_and_bumps_count() {
    let existing = PendingWake {
        work_id: WorkId::from("work-1"),
        obligation_json: ["tests"].into_iter().map(str::to_owned).collect(),
        count: 1,
        latest_reason: "first".to_owned(),
        merged_at: SystemTime::UNIX_EPOCH,
    };

    let merged = merge_wake(
        Some(&existing),
        "second",
        &["docs".to_owned(), "tests".to_owned()],
        SystemTime::UNIX_EPOCH + Duration::from_secs(10),
        WorkId::from("work-ignored"),
    );

    assert_eq!(merged.count, 2);
    assert_eq!(merged.latest_reason, "second");
    assert_eq!(merged.obligation_json.len(), 2);
    assert!(merged.obligation_json.contains("tests"));
    assert!(merged.obligation_json.contains("docs"));
}

#[test]
fn advance_session_resumes_same_work_session() {
    let existing = session("session-1", "runtime-1", "work-1", "agent-1", "/repo");
    let mut candidate = session("session-2", "runtime-2", "work-1", "agent-1", "/repo");
    candidate.last_record_id = Some(RecordId::from("record-9"));
    candidate.last_decision_summary = Some("accepted".to_owned());

    let resumed = advance_session(Some(&existing), candidate, false);

    assert_eq!(resumed.session_id, SessionId::from("session-1"));
    assert_eq!(resumed.runtime_session_id, "runtime-1");
    assert_eq!(resumed.last_record_id, Some(RecordId::from("record-9")));
    assert_eq!(resumed.last_decision_summary.as_deref(), Some("accepted"));
}

#[test]
fn advance_session_resets_on_invalid_runtime_session() {
    let existing = session("session-1", "runtime-1", "work-1", "agent-1", "/repo");
    let candidate = session("session-2", "runtime-2", "work-1", "agent-1", "/repo");

    let reset = advance_session(Some(&existing), candidate.clone(), true);

    assert_eq!(reset.session_id, candidate.session_id);
    assert_eq!(reset.runtime_session_id, candidate.runtime_session_id);
}

#[test]
fn replay_reconstructs_snapshot_from_transition_chain() {
    let contract = contract(vec![
        rule(
            TransitionKind::Queue,
            ActorKind::Board,
            vec![WorkStatus::Backlog],
            WorkStatus::Todo,
            LeaseEffect::None,
            vec![GateSpec::ExpectedRevMatchesSnapshot],
        ),
        rule(
            TransitionKind::Claim,
            ActorKind::Agent,
            vec![WorkStatus::Todo],
            WorkStatus::Doing,
            LeaseEffect::Acquire,
            vec![GateSpec::NoOpenLease, GateSpec::ExpectedRevMatchesSnapshot],
        ),
        rule(
            TransitionKind::Complete,
            ActorKind::Agent,
            vec![WorkStatus::Doing],
            WorkStatus::Done,
            LeaseEffect::Release,
            vec![
                GateSpec::LeasePresent,
                GateSpec::LeaseHeldByActor,
                GateSpec::ChangedFilesObserved,
            ],
        ),
    ]);
    let evidence = EvidenceBundle {
        changed_files: vec![FileChange {
            path: "src/lib.rs".to_owned(),
            change_kind: ChangeKind::Modified,
        }],
        ..EvidenceBundle::default()
    };

    let mut snapshot = snapshot(WorkStatus::Backlog, None, 0);
    let queue_intent = board_intent(TransitionKind::Queue, snapshot.rev);
    let queue_decision = decide_transition(
        &snapshot,
        None,
        None,
        &contract,
        &EvidenceBundle::default(),
        &queue_intent,
    );
    snapshot = queue_decision.next_snapshot.expect("queue should project");

    let claim_intent = intent(TransitionKind::Claim, snapshot.rev, "lease-1", None);
    let claim_decision = decide_transition(
        &snapshot,
        None,
        None,
        &contract,
        &EvidenceBundle::default(),
        &claim_intent,
    );
    snapshot = claim_decision.next_snapshot.expect("claim should project");

    let lease = lease("lease-1", "agent-1");
    let complete_intent = intent(TransitionKind::Complete, snapshot.rev, "lease-1", None);
    let complete_decision = decide_transition(
        &snapshot,
        Some(&lease),
        None,
        &contract,
        &evidence,
        &complete_intent,
    );
    let final_snapshot = complete_decision
        .next_snapshot
        .expect("complete should project");

    assert_eq!(queue_decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(claim_decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(complete_decision.outcome, DecisionOutcome::Accepted);
    assert_eq!(final_snapshot.status, WorkStatus::Done);
    assert_eq!(final_snapshot.rev, 3);
    assert_eq!(final_snapshot.active_lease_id, None);
}

#[test]
fn replay_reconstructs_timeout_requeue_snapshot_chain() {
    let mut snapshot = snapshot(WorkStatus::Backlog, None, 0);

    let queue_record = TransitionRecord {
        record_id: RecordId::from("record-queue"),
        company_id: CompanyId::from("company-1"),
        work_id: WorkId::from("work-1"),
        actor_kind: ActorKind::Board,
        actor_id: crate::model::ActorId::from("board"),
        lease_id: None,
        expected_rev: 0,
        before_status: WorkStatus::Backlog,
        after_status: Some(WorkStatus::Todo),
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::Queue,
        patch: crate::model::WorkPatch::default(),
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        evidence_inline: Some(crate::model::EvidenceInline {
            summary: "queue".to_owned(),
        }),
        evidence_refs: Vec::new(),
        happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
    };
    snapshot = replay_snapshot_from_records(&snapshot, std::slice::from_ref(&queue_record))
        .expect("queue replay should succeed");

    let claim_record = TransitionRecord {
        record_id: RecordId::from("record-claim"),
        company_id: CompanyId::from("company-1"),
        work_id: WorkId::from("work-1"),
        actor_kind: ActorKind::Agent,
        actor_id: crate::model::ActorId::from("agent-1"),
        lease_id: Some(LeaseId::from("lease-1")),
        expected_rev: 1,
        before_status: WorkStatus::Todo,
        after_status: Some(WorkStatus::Doing),
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::Claim,
        patch: crate::model::WorkPatch::default(),
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        evidence_inline: Some(crate::model::EvidenceInline {
            summary: "claim".to_owned(),
        }),
        evidence_refs: Vec::new(),
        happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
    };
    snapshot = replay_snapshot_from_records(&snapshot, std::slice::from_ref(&claim_record))
        .expect("claim replay should succeed");

    let timeout_record = TransitionRecord {
        record_id: RecordId::from("record-timeout"),
        company_id: CompanyId::from("company-1"),
        work_id: WorkId::from("work-1"),
        actor_kind: ActorKind::System,
        actor_id: crate::model::ActorId::from("system"),
        lease_id: Some(LeaseId::from("lease-1")),
        expected_rev: 2,
        before_status: WorkStatus::Doing,
        after_status: Some(WorkStatus::Todo),
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::TimeoutRequeue,
        patch: crate::model::WorkPatch {
            summary: "timed out run run-1".to_owned(),
            resolved_obligations: Vec::new(),
            declared_risks: Vec::new(),
        },
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        evidence_inline: Some(crate::model::EvidenceInline {
            summary: "timeout".to_owned(),
        }),
        evidence_refs: Vec::new(),
        happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(3),
    };
    let replayed = replay_snapshot_from_records(&snapshot, std::slice::from_ref(&timeout_record))
        .expect("timeout replay should succeed");

    assert_eq!(replayed.status, WorkStatus::Todo);
    assert_eq!(replayed.rev, 3);
    assert_eq!(replayed.active_lease_id, None);
    assert_eq!(replayed.assignee_agent_id, None);
}

#[test]
fn replay_rejects_accepted_record_without_after_status() {
    let base = snapshot(WorkStatus::Doing, Some("lease-1"), 2);
    let record = TransitionRecord {
        record_id: RecordId::from("record-missing-after"),
        company_id: CompanyId::from("company-1"),
        work_id: WorkId::from("work-1"),
        actor_kind: ActorKind::Agent,
        actor_id: crate::model::ActorId::from("agent-1"),
        lease_id: Some(LeaseId::from("lease-1")),
        expected_rev: 2,
        before_status: WorkStatus::Doing,
        after_status: None,
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::Complete,
        patch: crate::model::WorkPatch::default(),
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        evidence_inline: Some(crate::model::EvidenceInline {
            summary: "complete".to_owned(),
        }),
        evidence_refs: Vec::new(),
        happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(3),
    };

    let error = replay_snapshot_from_records(&base, &[record])
        .expect_err("accepted replay without after_status should fail");

    assert!(error.message.contains("missing after_status"));
}

#[test]
fn replay_rejects_record_before_status_mismatch() {
    let base = snapshot(WorkStatus::Todo, None, 1);
    let record = TransitionRecord {
        record_id: RecordId::from("record-before-mismatch"),
        company_id: CompanyId::from("company-1"),
        work_id: WorkId::from("work-1"),
        actor_kind: ActorKind::Board,
        actor_id: crate::model::ActorId::from("board"),
        lease_id: None,
        expected_rev: 1,
        before_status: WorkStatus::Doing,
        after_status: Some(WorkStatus::Done),
        outcome: DecisionOutcome::Accepted,
        reasons: Vec::new(),
        kind: TransitionKind::OverrideComplete,
        patch: crate::model::WorkPatch::default(),
        gate_results: Vec::new(),
        evidence: EvidenceBundle::default(),
        evidence_inline: Some(crate::model::EvidenceInline {
            summary: "override".to_owned(),
        }),
        evidence_refs: Vec::new(),
        happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
    };

    let error = replay_snapshot_from_records(&base, &[record])
        .expect_err("before_status mismatch should fail replay");

    assert!(error.message.contains("before_status does not match"));
}

fn snapshot(status: WorkStatus, active_lease_id: Option<&str>, rev: Rev) -> WorkSnapshot {
    WorkSnapshot {
        work_id: WorkId::from("work-1"),
        company_id: CompanyId::from("company-1"),
        parent_id: None,
        kind: WorkKind::Task,
        title: "Title".to_owned(),
        body: "Body".to_owned(),
        status,
        priority: crate::model::Priority::High,
        assignee_agent_id: Some(AgentId::from("agent-1")),
        active_lease_id: active_lease_id.map(LeaseId::from),
        rev,
        contract_set_id: ContractSetId::from("contract-1"),
        contract_rev: 1,
        created_at: SystemTime::UNIX_EPOCH,
        updated_at: SystemTime::UNIX_EPOCH,
    }
}

fn lease(lease_id: &str, agent_id: &str) -> WorkLease {
    WorkLease {
        lease_id: LeaseId::from(lease_id),
        company_id: CompanyId::from("company-1"),
        work_id: WorkId::from("work-1"),
        agent_id: AgentId::from(agent_id),
        run_id: None,
        acquired_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
        released_at: None,
        release_reason: None,
    }
}

fn intent(
    kind: TransitionKind,
    expected_rev: Rev,
    lease_id: &str,
    note: Option<&str>,
) -> TransitionIntent {
    TransitionIntent {
        work_id: WorkId::from("work-1"),
        agent_id: AgentId::from("agent-1"),
        lease_id: LeaseId::from(lease_id),
        expected_rev,
        kind,
        patch: crate::model::WorkPatch {
            summary: "summary".to_owned(),
            resolved_obligations: Vec::new(),
            declared_risks: Vec::new(),
        },
        note: note.map(str::to_owned),
        proof_hints: vec![ProofHint {
            kind: ProofHintKind::Summary,
            value: "summary".to_owned(),
        }],
    }
}

fn board_intent(kind: TransitionKind, expected_rev: Rev) -> TransitionIntent {
    TransitionIntent {
        work_id: WorkId::from("work-1"),
        agent_id: AgentId::from("agent-1"),
        lease_id: LeaseId::from("board-lease"),
        expected_rev,
        kind,
        patch: crate::model::WorkPatch::default(),
        note: Some("board action".to_owned()),
        proof_hints: vec![ProofHint {
            kind: ProofHintKind::Summary,
            value: "board action".to_owned(),
        }],
    }
}

fn contract(rules: Vec<TransitionRule>) -> ContractSet {
    ContractSet {
        contract_set_id: ContractSetId::from("contract-1"),
        company_id: CompanyId::from("company-1"),
        revision: 1,
        name: "Default".to_owned(),
        status: ContractSetStatus::Active,
        rules,
    }
}

fn rule(
    kind: TransitionKind,
    actor_kind: ActorKind,
    from: Vec<WorkStatus>,
    to: WorkStatus,
    lease_effect: LeaseEffect,
    gates: Vec<GateSpec>,
) -> TransitionRule {
    TransitionRule {
        kind,
        actor_kind,
        from,
        to,
        lease_effect,
        gates,
    }
}

fn session(
    session_id: &str,
    runtime_session_id: &str,
    work_id: &str,
    agent_id: &str,
    cwd: &str,
) -> TaskSession {
    TaskSession {
        session_id: SessionId::from(session_id),
        company_id: CompanyId::from("company-1"),
        agent_id: AgentId::from(agent_id),
        work_id: WorkId::from(work_id),
        runtime: RuntimeKind::Coclai,
        runtime_session_id: runtime_session_id.to_owned(),
        cwd: cwd.to_owned(),
        contract_rev: 1,
        last_record_id: None,
        last_decision_summary: None,
        last_gate_summary: Some("gate".to_owned()),
        updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(30),
    }
}
