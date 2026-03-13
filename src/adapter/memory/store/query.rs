use super::*;

pub(super) fn read_board(store: &MemoryStore) -> BoardReadModel {
    let state = store.state.borrow();

    let running_runs = state
        .runs
        .values()
        .filter(|run| run.status == RunStatus::Running)
        .map(|run| RunningRunView {
            run_id: run.run_id.as_str().to_owned(),
            agent_id: run.agent_id.as_str().to_owned(),
            work_id: run.work_id.as_str().to_owned(),
            lease_id: state
                .leases
                .values()
                .find(|lease| {
                    lease.released_at.is_none()
                        && lease.run_id.as_ref() == Some(&run.run_id)
                        && lease.work_id == run.work_id
                        && lease.agent_id == run.agent_id
                })
                .map(|lease| lease.lease_id.as_str().to_owned()),
        })
        .collect::<Vec<_>>();
    let running_agents = running_runs
        .iter()
        .map(|run| run.agent_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let pending_wakes = state
        .pending_wakes
        .values()
        .map(|wake| wake.work_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let pending_wake_details = state
        .pending_wakes
        .values()
        .map(|wake| PendingWakeSummaryView {
            work_id: wake.work_id.as_str().to_owned(),
            count: wake.count,
            latest_reason: wake.latest_reason.clone(),
            obligations: wake.obligation_json.iter().cloned().collect(),
        })
        .collect::<Vec<_>>();
    let blocked_work = state
        .snapshots
        .values()
        .filter(|snapshot| snapshot.status == WorkStatus::Blocked)
        .map(|snapshot| snapshot.work_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let recent_transition_records = state
        .transition_records
        .iter()
        .rev()
        .take(5)
        .map(|record| record.record_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let recent_transition_details = state
        .transition_records
        .iter()
        .rev()
        .take(5)
        .map(|record| {
            store_support::board_transition_detail(
                record.record_id.as_str(),
                record.work_id.as_str(),
                transition_kind_label(record.kind),
                decision_outcome_label(record.outcome),
                store_support::transition_summary(record.evidence_inline.as_ref(), &record.patch),
            )
        })
        .collect::<Vec<_>>();
    let recent_gate_failures = state
        .transition_records
        .iter()
        .rev()
        .filter(|record| store_support::is_gate_failure(record.outcome, &record.gate_results))
        .take(5)
        .map(|record| record.record_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let recent_gate_failure_details = state
        .transition_records
        .iter()
        .rev()
        .filter(|record| store_support::is_gate_failure(record.outcome, &record.gate_results))
        .take(5)
        .map(|record| {
            store_support::board_gate_failure_detail(
                record.record_id.as_str(),
                record.work_id.as_str(),
                decision_outcome_label(record.outcome),
                store_support::failed_gate_details(&record.gate_results),
            )
        })
        .collect::<Vec<_>>();

    BoardReadModel {
        running_agents,
        running_runs,
        pending_wakes,
        pending_wake_details,
        blocked_work,
        recent_transition_records,
        recent_transition_details,
        recent_gate_failures,
        recent_gate_failure_details,
        consumption_summary: consumption_summary(&state.consumption_events),
    }
}

pub(super) fn read_companies(store: &MemoryStore) -> CompanyReadModel {
    let state = store.state.borrow();
    CompanyReadModel {
        items: state
            .companies
            .values()
            .map(|company| {
                let active_contract = active_contract_for_company(&state, &company.company_id);
                CompanySummaryView {
                    company_id: company.company_id.as_str().to_owned(),
                    name: company.name.clone(),
                    description: company.description.clone(),
                    runtime_hard_stop_cents: company.runtime_hard_stop_cents,
                    active_contract_set_id: active_contract
                        .as_ref()
                        .map(|contract| contract.contract_set_id.as_str().to_owned()),
                    active_contract_revision: active_contract
                        .as_ref()
                        .map(|contract| contract.revision),
                    agent_count: state
                        .agents
                        .values()
                        .filter(|agent| agent.company_id == company.company_id)
                        .count(),
                    work_count: state
                        .snapshots
                        .values()
                        .filter(|snapshot| snapshot.company_id == company.company_id)
                        .count(),
                }
            })
            .collect(),
    }
}

pub(super) fn read_work(
    store: &MemoryStore,
    work_id: Option<&WorkId>,
) -> Result<WorkReadModel, StoreError> {
    let state = store.state.borrow();
    let items = if let Some(work_id) = work_id {
        vec![work_summary_for(&state, work_id).ok_or_else(|| not_found("read_work", work_id))?]
    } else {
        state
            .snapshots
            .values()
            .map(|snapshot| {
                let audit_entries = work_activity_entries(&state, &snapshot.work_id);
                work_summary_with_comments(
                    snapshot,
                    state.pending_wakes.get(snapshot.work_id.as_str()),
                    &state.contract_history,
                    &state.comments,
                    &audit_entries,
                )
            })
            .collect::<Vec<_>>()
    };

    Ok(WorkReadModel { items })
}

pub(super) fn read_agents(store: &MemoryStore) -> AgentReadModel {
    let state = store.state.borrow();
    let active_agents = state
        .leases
        .values()
        .filter(|lease| lease.released_at.is_none())
        .map(|lease| lease.agent_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut recent_runs = state.runs.values().cloned().collect::<Vec<_>>();
    recent_runs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    let mut current_sessions = state.sessions.values().cloned().collect::<Vec<_>>();
    current_sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

    AgentReadModel {
        active_agents,
        registered_agents: state
            .agents
            .values()
            .map(|agent| AgentSummaryView {
                agent_id: agent.agent_id.as_str().to_owned(),
                company_id: agent.company_id.as_str().to_owned(),
                name: agent.name.clone(),
                role: agent.role.clone(),
                status: agent.status,
            })
            .collect(),
        recent_runs: recent_runs
            .into_iter()
            .take(10)
            .map(|run| AgentRunView {
                run_id: run.run_id.as_str().to_owned(),
                agent_id: run.agent_id.as_str().to_owned(),
                work_id: run.work_id.as_str().to_owned(),
                status: run_status_label(run.status).to_owned(),
            })
            .collect(),
        current_sessions: current_sessions
            .into_iter()
            .map(|session| AgentSessionSummaryView {
                agent_id: session.agent_id.as_str().to_owned(),
                work_id: session.work_id.as_str().to_owned(),
                runtime: session.runtime,
                runtime_session_id: session.runtime_session_id,
                cwd: session.cwd,
                contract_rev: session.contract_rev,
                last_decision_summary: session.last_decision_summary,
                last_gate_summary: session.last_gate_summary,
            })
            .collect(),
        consumption_by_agent: agent_consumption_summaries(&state),
    }
}

pub(super) fn read_activity(store: &MemoryStore) -> ActivityReadModel {
    let state = store.state.borrow();
    ActivityReadModel {
        entries: activity_entries(&state),
    }
}

pub(super) fn read_run(store: &MemoryStore, run_id: &RunId) -> Result<RunReadModel, StoreError> {
    let state = store.state.borrow();
    let run = state
        .runs
        .get(run_id.as_str())
        .ok_or_else(|| run_not_found("read_run", run_id))?;
    let current_session = state
        .sessions
        .values()
        .find(|session| session.agent_id == run.agent_id && session.work_id == run.work_id)
        .map(|session| AgentSessionSummaryView {
            agent_id: session.agent_id.as_str().to_owned(),
            work_id: session.work_id.as_str().to_owned(),
            runtime: session.runtime,
            runtime_session_id: session.runtime_session_id.clone(),
            cwd: session.cwd.clone(),
            contract_rev: session.contract_rev,
            last_decision_summary: session.last_decision_summary.clone(),
            last_gate_summary: session.last_gate_summary.clone(),
        });

    Ok(RunReadModel {
        run_id: run.run_id.as_str().to_owned(),
        agent_id: run.agent_id.as_str().to_owned(),
        work_id: run.work_id.as_str().to_owned(),
        status: run_status_label(run.status).to_owned(),
        current_session,
    })
}

pub(super) fn read_contracts(store: &MemoryStore) -> ContractsReadModel {
    let state = store.state.borrow();
    ContractsReadModel {
        contract_set_id: state.contract.contract_set_id.as_str().to_owned(),
        name: state.contract.name.clone(),
        revision: state.contract.revision,
        status: state.contract.status,
        revisions: state
            .contract_history
            .iter()
            .map(|contract| ContractRevisionView {
                revision: contract.revision,
                status: contract.status,
                name: contract.name.clone(),
            })
            .collect(),
        rules: state.contract.rules.clone(),
    }
}
