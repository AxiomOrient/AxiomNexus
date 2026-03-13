use std::time::Duration;

use crate::{
    kernel,
    model::{
        ActorId, ActorKind, AgentId, DecisionOutcome, EvidenceBundle, EvidenceInline, LeaseId,
        RecordId, TransitionIntent, TransitionKind, TransitionRecord, WorkId, WorkPatch,
        WorkStatus,
    },
    port::store::{CommandStorePort, CommitDecisionReq, StoreError, StoreErrorKind},
};

use super::DECISION_PATH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimWorkCmd {
    pub(crate) work_id: WorkId,
    pub(crate) agent_id: AgentId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimWorkAck {
    pub(crate) transition_path: &'static str,
    pub(crate) snapshot_status: WorkStatus,
}

pub(crate) fn handle_claim_work(
    store: &impl CommandStorePort,
    cmd: ClaimWorkCmd,
) -> Result<ClaimWorkAck, StoreError> {
    let context = store.load_context(&cmd.work_id)?;
    let intent = claim_intent(&context.snapshot, &cmd);
    let evidence = claim_evidence(store, &cmd.agent_id)?;
    let decision = kernel::decide_transition(
        &context.snapshot,
        context.lease.as_ref(),
        context.pending_wake.as_ref(),
        &context.contract,
        &evidence,
        &intent,
    );

    match decision.outcome {
        DecisionOutcome::Accepted => {
            let happened_at = context.snapshot.updated_at + Duration::from_secs(1);
            let record = claim_record(&context, &intent, &decision, happened_at);
            store.commit_decision(CommitDecisionReq::new(decision.clone(), record, None))?;

            Ok(ClaimWorkAck {
                transition_path: DECISION_PATH,
                snapshot_status: decision
                    .next_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.status)
                    .unwrap_or(WorkStatus::Todo),
            })
        }
        DecisionOutcome::Rejected | DecisionOutcome::Conflict => {
            let happened_at = context.snapshot.updated_at + Duration::from_secs(1);
            let record = claim_record(&context, &intent, &decision, happened_at);
            store.commit_decision(CommitDecisionReq::new(decision.clone(), record, None))?;
            Err(StoreError {
                kind: StoreErrorKind::Conflict,
                message: decision.summary,
            })
        }
        DecisionOutcome::OverrideAccepted => unreachable!("claim cannot override accept"),
    }
}

fn claim_intent(snapshot: &crate::model::WorkSnapshot, cmd: &ClaimWorkCmd) -> TransitionIntent {
    TransitionIntent {
        work_id: cmd.work_id.clone(),
        agent_id: cmd.agent_id.clone(),
        lease_id: LeaseId::from(format!(
            "lease-{}-rev-{}",
            snapshot.work_id,
            snapshot.rev + 1
        )),
        expected_rev: snapshot.rev,
        kind: TransitionKind::Claim,
        patch: WorkPatch::default(),
        note: None,
        proof_hints: Vec::new(),
    }
}

fn claim_evidence(
    store: &impl CommandStorePort,
    agent_id: &AgentId,
) -> Result<EvidenceBundle, StoreError> {
    let agent = store.load_agent_facts(agent_id)?;

    Ok(EvidenceBundle {
        observed_agent_status: agent.as_ref().map(|agent| agent.status),
        observed_agent_company_id: agent.map(|agent| agent.company_id),
        ..EvidenceBundle::default()
    })
}

fn claim_record(
    context: &crate::port::store::WorkContext,
    intent: &TransitionIntent,
    decision: &crate::model::TransitionDecision,
    happened_at: std::time::SystemTime,
) -> TransitionRecord {
    TransitionRecord {
        record_id: RecordId::from(format!(
            "record-{}-{}-claim",
            context.snapshot.work_id,
            context.snapshot.rev + 1
        )),
        company_id: context.snapshot.company_id.clone(),
        work_id: context.snapshot.work_id.clone(),
        actor_kind: ActorKind::Agent,
        actor_id: ActorId::from(intent.agent_id.as_str()),
        run_id: context
            .lease
            .as_ref()
            .and_then(|lease| lease.run_id.clone()),
        session_id: None,
        lease_id: (decision.outcome == DecisionOutcome::Accepted).then(|| intent.lease_id.clone()),
        expected_rev: context.snapshot.rev,
        contract_set_id: context.snapshot.contract_set_id.clone(),
        contract_rev: context.snapshot.contract_rev,
        before_status: context.snapshot.status,
        after_status: decision
            .next_snapshot
            .as_ref()
            .map(|snapshot| snapshot.status),
        outcome: decision.outcome,
        reasons: decision.reasons.clone(),
        kind: TransitionKind::Claim,
        patch: WorkPatch::default(),
        gate_results: decision.gate_results.clone(),
        evidence: decision.evidence.clone(),
        evidence_inline: Some(EvidenceInline {
            summary: decision.summary.clone(),
        }),
        evidence_refs: decision.evidence.artifact_refs.clone(),
        happened_at,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_AGENT_ID, DEMO_TODO_WORK_ID},
        app::cmd::{
            activate_contract::{handle_activate_contract, ActivateContractCmd},
            create_agent::{handle_create_agent, CreateAgentCmd},
            create_company::{handle_create_company, CreateCompanyCmd},
            create_contract_draft::{handle_create_contract_draft, CreateContractDraftCmd},
            create_work::{handle_create_work, CreateWorkCmd},
        },
        model::{CompanyId, ContractSetId, TransitionKind, WorkKind},
        port::store::StorePort,
    };

    use super::{handle_claim_work, ClaimWorkCmd};

    #[test]
    fn claim_work_uses_kernel_decision_and_persists_claimed_snapshot() {
        let store = MemoryStore::demo();

        let ack = handle_claim_work(
            &store,
            ClaimWorkCmd {
                work_id: crate::model::WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            },
        )
        .expect("claim work should succeed");

        let activity = store.read_activity();
        let work = store
            .read_work(Some(&crate::model::WorkId::from(DEMO_TODO_WORK_ID)))
            .expect("work should remain readable");

        assert_eq!(ack.snapshot_status, crate::model::WorkStatus::Doing);
        assert!(activity.entries.len() >= 2);
        assert!(activity.entries.iter().any(|entry| {
            entry.event_kind == "transition"
                && entry.after_status == Some(crate::model::WorkStatus::Doing)
        }));
        assert_eq!(work.items[0].status, crate::model::WorkStatus::Doing);
        assert!(work.items[0].active_lease_id.is_some());
    }

    #[test]
    fn claim_work_records_conflict_when_agent_is_not_runnable() {
        let store = MemoryStore::demo();

        let error = handle_claim_work(
            &store,
            ClaimWorkCmd {
                work_id: crate::model::WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: crate::model::AgentId::from("00000000-0000-4000-8000-000000000004"),
            },
        )
        .expect_err("paused claim should conflict");

        let activity = store.read_activity();

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Conflict);
        assert!(error.message.contains("contract gates failed"));
        assert!(activity.entries.iter().any(|entry| {
            entry.event_kind == "transition"
                && entry.outcome.as_deref() == Some("rejected")
                && entry.work_id == DEMO_TODO_WORK_ID
        }));
    }

    #[test]
    fn claim_work_rejects_when_pinned_contract_has_no_claim_rule() {
        let store = MemoryStore::demo();
        let company = handle_create_company(
            &store,
            CreateCompanyCmd {
                name: "No Claim Co".to_owned(),
                description: "company without claim rule".to_owned(),
                runtime_hard_stop_cents: None,
            },
        )
        .expect("company should create");
        let draft = handle_create_contract_draft(
            &store,
            CreateContractDraftCmd {
                company_id: CompanyId::from(company.company_id.as_str()),
                name: "no-claim-contract".to_owned(),
                rules: store
                    .read_contracts()
                    .rules
                    .into_iter()
                    .filter(|rule| rule.kind != TransitionKind::Claim)
                    .collect(),
            },
        )
        .expect("draft should create");
        handle_activate_contract(
            &store,
            ActivateContractCmd {
                company_id: CompanyId::from(company.company_id.as_str()),
                revision: draft.revision,
            },
        )
        .expect("draft should activate");
        let contract_set_id = store
            .read_companies()
            .items
            .into_iter()
            .find(|item| item.company_id == company.company_id)
            .and_then(|item| item.active_contract_set_id)
            .expect("new company should expose active contract set");
        let agent = handle_create_agent(
            &store,
            CreateAgentCmd {
                company_id: CompanyId::from(company.company_id.as_str()),
                name: "No Claim Agent".to_owned(),
                role: "implementer".to_owned(),
            },
        )
        .expect("agent should create");
        let work = handle_create_work(
            &store,
            CreateWorkCmd {
                company_id: CompanyId::from(company.company_id.as_str()),
                parent_id: None,
                kind: WorkKind::Task,
                title: "No claim work".to_owned(),
                body: String::new(),
                contract_set_id: ContractSetId::from(contract_set_id.as_str()),
            },
        )
        .expect("work should create");

        let error = handle_claim_work(
            &store,
            ClaimWorkCmd {
                work_id: crate::model::WorkId::from(work.work_id.as_str()),
                agent_id: crate::model::AgentId::from(agent.agent_id.as_str()),
            },
        )
        .expect_err("claim should fail without contract rule");

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Conflict);
        assert!(error.message.contains("no matching transition rule"));
        let created = store
            .read_work(Some(&crate::model::WorkId::from(work.work_id.as_str())))
            .expect("work should still read");
        assert_eq!(created.items[0].status, crate::model::WorkStatus::Backlog);
    }
}
