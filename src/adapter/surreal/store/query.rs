use super::*;

pub(super) fn read_board(store: &SurrealStore) -> BoardReadModel {
    let runs = store
        .query_docs_with_bind::<RunDoc, _>(
            "SELECT * FROM run WHERE status = $status ORDER BY updated_at_secs DESC",
            "status",
            run_status_label(RunStatus::Running).to_owned(),
        )
        .unwrap_or_default();
    let leases = store
        .query_docs::<LeaseRunDoc>(
            "SELECT lease_id, run_id, work_id, agent_id, released_at_secs FROM lease",
        )
        .unwrap_or_default();
    let pending_wakes = store
        .select_table::<PendingWakeDoc>("pending_wake")
        .unwrap_or_default();
    let blocked_work = store
        .query_docs_with_bind::<WorkDoc, _>(
            "SELECT * FROM work WHERE status = $status ORDER BY updated_at_secs DESC",
            "status",
            work_status_label(WorkStatus::Blocked).to_owned(),
        )
        .unwrap_or_default();
    let recent_records = store
        .query_docs::<TransitionRecordDoc>(
            "SELECT * FROM transition_record ORDER BY happened_at_secs DESC LIMIT 5",
        )
        .unwrap_or_default();
    let recent_failure_candidates = store
        .query_docs::<TransitionRecordDoc>(
            "SELECT * FROM transition_record ORDER BY happened_at_secs DESC LIMIT 100",
        )
        .unwrap_or_default();
    let consumption = store
        .query_docs::<ConsumptionAgentDoc>(
            "SELECT agent_id, input_tokens, output_tokens, run_seconds, estimated_cost_cents FROM consumption_event",
        )
        .unwrap_or_default();

    let running_runs = runs
        .iter()
        .filter(|run| run.status == run_status_label(RunStatus::Running))
        .map(|run| RunningRunView {
            run_id: run.run_id.clone(),
            agent_id: run.agent_id.clone(),
            work_id: run.work_id.clone(),
            lease_id: leases
                .iter()
                .find(|lease| {
                    lease.released_at_secs.is_none()
                        && lease.run_id.as_deref() == Some(run.run_id.as_str())
                        && lease.work_id == run.work_id
                        && lease.agent_id == run.agent_id
                })
                .map(|lease| lease.lease_id.clone()),
        })
        .collect::<Vec<_>>();
    let running_agents = running_runs
        .iter()
        .map(|run| run.agent_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let recent_gate_failure_details = recent_failure_candidates
        .iter()
        .filter(|record| record.outcome == "rejected" || !record.failed_gates.is_empty())
        .take(5)
        .map(|record| {
            store_support::board_gate_failure_detail(
                record.record_id.clone(),
                record.work_id.clone(),
                record.outcome.clone(),
                record.failed_gates.clone(),
            )
        })
        .collect::<Vec<_>>();

    BoardReadModel {
        running_agents,
        running_runs,
        pending_wakes: pending_wakes
            .iter()
            .map(|wake| wake.work_id.clone())
            .collect(),
        pending_wake_details: pending_wakes
            .iter()
            .map(|wake| PendingWakeSummaryView {
                work_id: wake.work_id.clone(),
                count: wake.count,
                latest_reason: wake.latest_reason.clone(),
                obligations: wake.obligations.clone(),
            })
            .collect(),
        blocked_work: blocked_work
            .iter()
            .map(|snapshot| snapshot.work_id.clone())
            .collect(),
        recent_transition_records: recent_records
            .iter()
            .map(|record| record.record_id.clone())
            .collect(),
        recent_transition_details: recent_records
            .iter()
            .map(|record| {
                store_support::board_transition_detail(
                    record.record_id.clone(),
                    record.work_id.clone(),
                    record.kind.clone(),
                    record.outcome.clone(),
                    record
                        .evidence_summary
                        .clone()
                        .unwrap_or_else(|| record.patch_summary.clone()),
                )
            })
            .collect(),
        recent_gate_failures: recent_gate_failure_details
            .iter()
            .map(|detail| detail.record_id.clone())
            .collect(),
        recent_gate_failure_details,
        consumption_summary: summarized_consumption(&consumption),
    }
}

pub(super) fn read_companies(store: &SurrealStore) -> CompanyReadModel {
    let companies = store
        .select_table::<CompanyDoc>("company")
        .unwrap_or_default();
    let agent_company_ids = store
        .query_docs::<CompanyKeyDoc>("SELECT company_id FROM agent")
        .unwrap_or_default();
    let work_company_ids = store
        .query_docs::<CompanyKeyDoc>("SELECT company_id FROM work")
        .unwrap_or_default();
    let active_contracts = store
        .query_docs_with_bind::<ActiveContractDoc, _>(
            "SELECT company_id, contract_set_id, revision FROM contract_revision WHERE status = $status",
            "status",
            contract_status_label(ContractSetStatus::Active).to_owned(),
        )
        .unwrap_or_default()
        .into_iter()
        .map(|contract| (contract.company_id.clone(), contract))
        .collect::<BTreeMap<_, _>>();
    let agent_counts = count_by_key(
        agent_company_ids
            .iter()
            .map(|agent| agent.company_id.as_str()),
    );
    let work_counts = count_by_key(work_company_ids.iter().map(|work| work.company_id.as_str()));

    CompanyReadModel {
        items: companies
            .into_iter()
            .map(|company| {
                let active_contract = active_contracts.get(company.company_id.as_str());
                CompanySummaryView {
                    company_id: company.company_id.clone(),
                    name: company.name,
                    description: company.description,
                    runtime_hard_stop_cents: company.runtime_hard_stop_cents,
                    active_contract_set_id: active_contract
                        .map(|contract| contract.contract_set_id.clone()),
                    active_contract_revision: active_contract.map(|contract| contract.revision),
                    agent_count: agent_counts
                        .get(company.company_id.as_str())
                        .copied()
                        .unwrap_or_default(),
                    work_count: work_counts
                        .get(company.company_id.as_str())
                        .copied()
                        .unwrap_or_default(),
                }
            })
            .collect(),
    }
}

pub(super) fn read_work(
    store: &SurrealStore,
    work_id: Option<&WorkId>,
) -> Result<WorkReadModel, StoreError> {
    if let Some(work_id) = work_id {
        let doc = store
            .select_record::<WorkDoc>(&work_record_id(work_id.as_str()))?
            .ok_or_else(|| not_found("read_work", work_id.as_str()))?;
        let contract = contract_for_work(store, &doc).ok();
        let pending_obligations = store
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(work_id.as_str()))?
            .map(|wake| wake.obligations)
            .unwrap_or_default();
        let comments = work_comments_for(store, work_id.as_str())?;
        let audit_entries = work_activity_entries(store, work_id.as_str())?;

        return Ok(WorkReadModel {
            items: vec![WorkSummary {
                work_id: doc.work_id.clone(),
                parent_id: doc.parent_id.clone(),
                kind: parse_work_kind(&doc.kind).unwrap_or(WorkKind::Task),
                title: doc.title.clone(),
                body: doc.body.clone(),
                status: parse_work_status(&doc.status).unwrap_or(WorkStatus::Backlog),
                rev: doc.rev,
                active_lease_id: doc.active_lease_id.clone(),
                contract_set_id: doc.contract_set_id.clone(),
                contract_rev: doc.contract_rev,
                contract_name: contract.as_ref().map(|contract| contract.name.clone()),
                contract_status: contract.as_ref().map(|contract| contract.status),
                pending_obligations,
                comments,
                audit_entries,
            }],
        });
    }

    let docs = store.select_table::<WorkDoc>("work")?;
    let contracts = store.select_table::<ContractRevisionDoc>("contract_revision")?;
    let contract_index = contracts
        .iter()
        .map(|contract| {
            (
                (contract.contract_set_id.as_str(), contract.revision),
                contract,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let pending_wakes = store
        .select_table::<PendingWakeDoc>("pending_wake")?
        .into_iter()
        .map(|wake| (wake.work_id, wake.obligations))
        .collect::<BTreeMap<_, _>>();
    let comments_by_work = grouped_work_comments(store.query_docs::<WorkCommentDoc>(
        "SELECT * FROM work_comment ORDER BY created_at_secs ASC",
    )?)?;
    let activity_by_work =
        grouped_work_activity_entries(store.select_table::<ActivityEventDoc>("activity_event")?)?;

    Ok(WorkReadModel {
        items: docs
            .into_iter()
            .map(|doc| {
                let contract = contract_index
                    .get(&(doc.contract_set_id.as_str(), doc.contract_rev))
                    .copied();
                WorkSummary {
                    work_id: doc.work_id.clone(),
                    parent_id: doc.parent_id.clone(),
                    kind: parse_work_kind(&doc.kind).unwrap_or(WorkKind::Task),
                    title: doc.title.clone(),
                    body: doc.body.clone(),
                    status: parse_work_status(&doc.status).unwrap_or(WorkStatus::Backlog),
                    rev: doc.rev,
                    active_lease_id: doc.active_lease_id.clone(),
                    contract_set_id: doc.contract_set_id.clone(),
                    contract_rev: doc.contract_rev,
                    contract_name: contract.map(|contract| contract.name.clone()),
                    contract_status: contract
                        .and_then(|contract| parse_contract_status(&contract.status).ok()),
                    pending_obligations: pending_wakes
                        .get(&doc.work_id)
                        .cloned()
                        .unwrap_or_default(),
                    comments: comments_by_work
                        .get(&doc.work_id)
                        .cloned()
                        .unwrap_or_default(),
                    audit_entries: activity_by_work
                        .get(&doc.work_id)
                        .cloned()
                        .unwrap_or_default(),
                }
            })
            .collect(),
    })
}

pub(super) fn read_agents(store: &SurrealStore) -> AgentReadModel {
    let lease_agents = store
        .query_docs::<LeaseAgentDoc>("SELECT agent_id, released_at_secs FROM lease")
        .unwrap_or_default();
    let agents = store.select_table::<AgentDoc>("agent").unwrap_or_default();
    let runs = store
        .query_docs::<RunDoc>("SELECT * FROM run ORDER BY updated_at_secs DESC LIMIT 10")
        .unwrap_or_default();
    let sessions = store
        .query_docs::<SessionDoc>("SELECT * FROM task_session ORDER BY updated_at_secs DESC")
        .unwrap_or_default();
    let consumption = store
        .query_docs::<ConsumptionAgentDoc>(
            "SELECT agent_id, input_tokens, output_tokens, run_seconds, estimated_cost_cents FROM consumption_event",
        )
        .unwrap_or_default();

    AgentReadModel {
        active_agents: lease_agents
            .iter()
            .filter(|lease| lease.released_at_secs.is_none())
            .map(|lease| lease.agent_id.clone())
            .collect(),
        registered_agents: agents
            .iter()
            .filter_map(|agent| {
                Some(AgentSummaryView {
                    agent_id: agent.agent_id.clone(),
                    company_id: agent.company_id.clone(),
                    name: agent.name.clone(),
                    role: agent.role.clone(),
                    status: parse_agent_status(&agent.status).ok()?,
                })
            })
            .collect(),
        recent_runs: runs
            .into_iter()
            .map(|run| AgentRunView {
                run_id: run.run_id,
                agent_id: run.agent_id,
                work_id: run.work_id,
                status: run.status,
            })
            .collect(),
        current_sessions: sessions
            .into_iter()
            .filter_map(|session| {
                Some(AgentSessionSummaryView {
                    agent_id: session.agent_id,
                    work_id: session.work_id,
                    runtime: parse_runtime_kind(&session.runtime).ok()?,
                    runtime_session_id: session.runtime_session_id,
                    cwd: session.cwd,
                    contract_rev: session.contract_rev,
                    last_decision_summary: session.last_decision_summary,
                    last_gate_summary: session.last_gate_summary,
                })
            })
            .collect(),
        consumption_by_agent: agent_consumption_summaries(&agents, &consumption),
    }
}

pub(super) fn read_activity(store: &SurrealStore) -> ActivityReadModel {
    ActivityReadModel {
        entries: activity_entries(store).unwrap_or_default(),
    }
}

pub(super) fn read_run(
    store: &SurrealStore,
    run_id: &crate::model::RunId,
) -> Result<RunReadModel, StoreError> {
    let run = store
        .select_record::<RunDoc>(&run_record_id(run_id.as_str()))?
        .ok_or_else(|| not_found("read_run", run_id.as_str()))?;
    let current_session = store
        .select_record::<SessionDoc>(&session_record_id_parts(&run.agent_id, &run.work_id))?
        .map(SessionDoc::into_agent_session_summary)
        .transpose()?;

    Ok(RunReadModel {
        run_id: run.run_id,
        agent_id: run.agent_id,
        work_id: run.work_id,
        status: run.status,
        current_session,
    })
}

pub(super) fn read_contracts(store: &SurrealStore) -> ContractsReadModel {
    let mut contracts = store
        .select_table::<ContractRevisionDoc>("contract_revision")
        .unwrap_or_default();
    contracts.sort_by_key(|contract| contract.revision);

    let selected = contracts
        .iter()
        .find(|contract| contract.status == contract_status_label(ContractSetStatus::Active))
        .or_else(|| contracts.last());

    if let Some(contract) = selected {
        ContractsReadModel {
            contract_set_id: contract.contract_set_id.clone(),
            name: contract.name.clone(),
            revision: contract.revision,
            status: parse_contract_status(&contract.status).unwrap_or(ContractSetStatus::Draft),
            revisions: contracts
                .iter()
                .map(|contract| ContractRevisionView {
                    revision: contract.revision,
                    status: parse_contract_status(&contract.status)
                        .unwrap_or(ContractSetStatus::Draft),
                    name: contract.name.clone(),
                })
                .collect(),
            rules: serde_json::from_value(contract.rules_json.clone()).unwrap_or_default(),
        }
    } else {
        ContractsReadModel {
            contract_set_id: String::new(),
            name: String::new(),
            revision: 0,
            status: ContractSetStatus::Draft,
            revisions: Vec::new(),
            rules: Vec::new(),
        }
    }
}
