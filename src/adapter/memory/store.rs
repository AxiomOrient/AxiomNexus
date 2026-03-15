use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};

mod commit;
mod query;

use crate::{
    adapter::store_support,
    kernel,
    model::{
        ActorId, ActorKind, AgentId, AgentStatus, BillingKind, CompanyId, CompanyProfile,
        ConsumptionUsage, ContractSet, ContractSetId, ContractSetStatus, LeaseEffect, LeaseId,
        LeaseReleaseReason, PendingWake, PendingWakeEffect, Priority, RunId, RunStatus,
        TaskSession, TransitionKind, TransitionRecord, TransitionRule, WorkId, WorkKind, WorkLease,
        WorkSnapshot, WorkStatus,
    },
    port::store::{
        ActivateContractReq, ActivateContractRes, ActivityEntryView, ActivityReadModel,
        AgentConsumptionSummaryView, AgentFacts, AgentReadModel, AgentRunView,
        AgentSessionSummaryView, AgentSummaryView, AppendCommentReq, AppendCommentRes,
        BoardReadModel, ClaimLeaseReq, ClaimLeaseRes, CommitDecisionReq, CommitDecisionRes,
        CompanyReadModel, CompanySummaryView, ConsumptionSummaryView, ContractRevisionView,
        ContractsReadModel, CreateAgentReq, CreateAgentRes, CreateCompanyReq, CreateCompanyRes,
        CreateContractDraftReq, CreateContractDraftRes, CreateWorkReq, CreateWorkRes, MergeWakeReq,
        PendingWakeSummaryView, QueuedRunCandidate, ReapedRun, RecordConsumptionReq, RunReadModel,
        RunningRunView, RuntimeTurnContext, SessionKey, SetAgentStatusReq, SetAgentStatusRes,
        StoreError, StoreErrorKind, StorePort, UpdateWorkReq, UpdateWorkRes, WorkCommentView,
        WorkContext, WorkReadModel, WorkSummary,
    },
};

pub(crate) const DEMO_COMPANY_ID: &str = "00000000-0000-4000-8000-000000000001";
pub(crate) const DEMO_AGENT_ID: &str = "00000000-0000-4000-8000-000000000002";
const DEMO_FOREIGN_COMPANY_ID: &str = "00000000-0000-4000-8000-000000000003";
const DEMO_PAUSED_AGENT_ID: &str = "00000000-0000-4000-8000-000000000004";
pub(crate) const DEMO_TERMINATED_AGENT_ID: &str = "00000000-0000-4000-8000-000000000005";
const DEMO_FOREIGN_AGENT_ID: &str = "00000000-0000-4000-8000-000000000006";
pub(crate) const DEMO_TODO_WORK_ID: &str = "00000000-0000-4000-8000-000000000011";
pub(crate) const DEMO_DOING_WORK_ID: &str = "00000000-0000-4000-8000-000000000012";
pub(crate) const DEMO_LEASE_ID: &str = "00000000-0000-4000-8000-000000000013";
pub(crate) const DEMO_CONTRACT_SET_ID: &str = "00000000-0000-4000-8000-000000000014";

fn default_next_agent_seq() -> u64 {
    6
}

fn default_next_work_seq() -> u64 {
    12
}

fn default_next_company_seq() -> u64 {
    3
}

fn default_next_contract_seq() -> u64 {
    14
}

pub(crate) struct MemoryStore {
    state: RefCell<MemoryState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryState {
    #[serde(default = "default_companies")]
    companies: BTreeMap<String, CompanyProfile>,
    agents: BTreeMap<String, RegisteredAgent>,
    runs: BTreeMap<String, RegisteredRun>,
    snapshots: BTreeMap<String, WorkSnapshot>,
    leases: BTreeMap<String, WorkLease>,
    pending_wakes: BTreeMap<String, PendingWake>,
    sessions: BTreeMap<String, TaskSession>,
    comments: Vec<PersistedComment>,
    consumption_events: Vec<PersistedConsumptionEvent>,
    transition_records: Vec<TransitionRecord>,
    activity_events: Vec<TimedActivityEntry>,
    contract: ContractSet,
    contract_history: Vec<ContractSet>,
    next_comment_seq: u64,
    next_consumption_seq: u64,
    #[serde(default = "default_next_agent_seq")]
    next_agent_seq: u64,
    #[serde(default = "default_next_company_seq")]
    next_company_seq: u64,
    #[serde(default = "default_next_contract_seq")]
    next_contract_seq: u64,
    #[serde(default = "default_next_work_seq")]
    next_work_seq: u64,
    next_run_seq: u64,
    next_lease_seq: u64,
    tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ContractTemplate {
    name: String,
    revision: u32,
    status: ContractSetStatus,
    rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegisteredAgent {
    agent_id: AgentId,
    company_id: CompanyId,
    #[serde(default)]
    name: String,
    #[serde(default)]
    role: String,
    status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegisteredRun {
    run_id: RunId,
    company_id: CompanyId,
    agent_id: AgentId,
    work_id: WorkId,
    status: RunStatus,
    created_at: SystemTime,
    updated_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedComment {
    comment_id: String,
    company_id: CompanyId,
    work_id: WorkId,
    author_kind: ActorKind,
    author_id: ActorId,
    source: Option<String>,
    body: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedConsumptionEvent {
    event_id: String,
    company_id: CompanyId,
    agent_id: AgentId,
    run_id: RunId,
    billing_kind: BillingKind,
    usage: ConsumptionUsage,
    created_at: SystemTime,
}

impl MemoryStore {
    pub(crate) fn demo() -> Self {
        Self {
            state: RefCell::new(MemoryState::demo()),
        }
    }

    fn with_state(state: MemoryState) -> Self {
        Self {
            state: RefCell::new(state),
        }
    }

    fn cloned_state(&self) -> MemoryState {
        self.state.borrow().clone()
    }

    fn replace_state(&self, next: MemoryState) -> Result<(), StoreError> {
        *self.state.borrow_mut() = next;
        Ok(())
    }

    pub(crate) fn claim_lease(&self, req: ClaimLeaseReq) -> Result<ClaimLeaseRes, StoreError> {
        let mut next = self.cloned_state();
        let acquired_at = next.next_timestamp();
        let snapshot = next
            .snapshots
            .get(req.work_id.as_str())
            .cloned()
            .ok_or_else(|| not_found("claim_lease", &req.work_id))?;
        let lease = acquire_claim_lease(
            &mut next,
            &req.work_id,
            &req.agent_id,
            &req.lease_id,
            acquired_at,
            "claim_lease",
        )?;
        let record = crate::kernel::claim_transition_record(&snapshot, &lease, acquired_at);
        let activity = activity_entry_from_transition(&record);
        next.transition_records.push(record);
        next.activity_events.push(activity);
        self.replace_state(next)?;

        Ok(ClaimLeaseRes { lease })
    }
}

#[cfg(test)]
pub(crate) fn demo_state_json() -> serde_json::Value {
    serde_json::to_value(MemoryState::demo()).expect("demo state should encode")
}

impl StorePort for MemoryStore {
    fn append_comment(&self, req: AppendCommentReq) -> Result<AppendCommentRes, StoreError> {
        let mut next = self.cloned_state();
        let snapshot = next
            .snapshots
            .get(req.work_id.as_str())
            .cloned()
            .ok_or_else(|| not_found("append_comment", &req.work_id))?;

        if snapshot.company_id != req.company_id {
            return Err(conflict(
                "append_comment rejects company/work boundary violations",
            ));
        }

        let comment_id = next.push_comment(
            req.company_id,
            req.work_id,
            req.author_kind,
            req.author_id,
            None,
            req.body,
        );
        self.replace_state(next)?;

        Ok(AppendCommentRes { comment_id })
    }

    fn create_agent(&self, req: CreateAgentReq) -> Result<CreateAgentRes, StoreError> {
        let mut next = self.cloned_state();

        if !next.companies.contains_key(req.company_id.as_str()) {
            return Err(conflict("create_agent requires a registered company"));
        }

        next.next_agent_seq += 1;
        let agent_id = AgentId::from(sequenced_id(next.next_agent_seq));
        next.agents.insert(
            agent_id.to_string(),
            RegisteredAgent {
                agent_id: agent_id.clone(),
                company_id: req.company_id,
                name: req.name,
                role: req.role,
                status: AgentStatus::Active,
            },
        );
        self.replace_state(next)?;

        Ok(CreateAgentRes {
            agent_id,
            status: AgentStatus::Active,
        })
    }

    fn create_company(&self, req: CreateCompanyReq) -> Result<CreateCompanyRes, StoreError> {
        let mut next = self.cloned_state();
        next.next_company_seq += 1;
        let profile = CompanyProfile {
            company_id: CompanyId::from(sequenced_id(next.next_company_seq)),
            name: req.name,
            description: req.description,
            runtime_hard_stop_cents: req.runtime_hard_stop_cents,
            recorded_estimated_cost_cents: 0,
        };
        next.companies
            .insert(profile.company_id.as_str().to_owned(), profile.clone());
        self.replace_state(next)?;

        Ok(CreateCompanyRes { profile })
    }

    fn create_work(&self, req: CreateWorkReq) -> Result<CreateWorkRes, StoreError> {
        let mut next = self.cloned_state();
        let active_contract = active_contract_for_company(&next, &req.company_id)
            .ok_or_else(|| conflict("create_work requires an active contract for the company"))?;

        if active_contract.contract_set_id != req.contract_set_id {
            return Err(conflict(
                "create_work requires the active contract set for the company",
            ));
        }

        let parent_id = match req.parent_id {
            Some(parent_id) => {
                let parent = next
                    .snapshots
                    .get(parent_id.as_str())
                    .ok_or_else(|| not_found("create_work parent", &parent_id))?;
                if parent.company_id != req.company_id {
                    return Err(conflict(
                        "create_work rejects parent/company boundary violations",
                    ));
                }
                if parent.contract_set_id != req.contract_set_id {
                    return Err(conflict(
                        "create_work requires parent and child to share contract set",
                    ));
                }
                Some(parent_id)
            }
            None => None,
        };

        next.next_work_seq += 1;
        let timestamp = next.next_timestamp();
        let snapshot = WorkSnapshot {
            work_id: WorkId::from(sequenced_id(next.next_work_seq)),
            company_id: req.company_id,
            parent_id,
            kind: req.kind,
            title: req.title,
            body: req.body,
            status: WorkStatus::Backlog,
            priority: Priority::Medium,
            assignee_agent_id: None,
            active_lease_id: None,
            rev: 0,
            contract_set_id: req.contract_set_id,
            contract_rev: active_contract.revision,
            created_at: timestamp,
            updated_at: timestamp,
        };
        next.snapshots
            .insert(snapshot.work_id.as_str().to_owned(), snapshot.clone());
        self.replace_state(next)?;

        Ok(CreateWorkRes { snapshot })
    }

    fn set_agent_status(&self, req: SetAgentStatusReq) -> Result<SetAgentStatusRes, StoreError> {
        let mut next = self.cloned_state();
        let agent = next
            .agents
            .get_mut(req.agent_id.as_str())
            .ok_or_else(|| not_found("set_agent_status", &req.agent_id))?;

        if agent.status == AgentStatus::Terminated && req.status != AgentStatus::Terminated {
            return Err(conflict("terminated agent cannot resume"));
        }

        agent.status = req.status;
        self.replace_state(next)?;

        Ok(SetAgentStatusRes {
            agent_id: req.agent_id,
            status: req.status,
        })
    }

    fn update_work(&self, req: UpdateWorkReq) -> Result<UpdateWorkRes, StoreError> {
        let mut next = self.cloned_state();
        let existing = next
            .snapshots
            .get(req.work_id.as_str())
            .cloned()
            .ok_or_else(|| not_found("update_work", &req.work_id))?;

        if let Some(parent_id) = req.parent_id.as_ref() {
            let parent = next
                .snapshots
                .get(parent_id.as_str())
                .ok_or_else(|| not_found("update_work parent", parent_id))?;
            if parent.company_id != existing.company_id {
                return Err(conflict(
                    "update_work rejects parent/company boundary violations",
                ));
            }
            if parent.contract_set_id != existing.contract_set_id {
                return Err(conflict(
                    "update_work requires parent and child to share contract set",
                ));
            }
            if parent_id == &req.work_id || would_create_work_cycle(&next, &req.work_id, parent_id)
            {
                return Err(conflict("update_work rejects tree cycles"));
            }
        }

        let updated_at = next.next_timestamp();
        let snapshot = next
            .snapshots
            .get_mut(req.work_id.as_str())
            .ok_or_else(|| not_found("update_work", &req.work_id))?;
        snapshot.parent_id = req.parent_id;
        snapshot.title = req.title;
        snapshot.body = req.body;
        snapshot.rev += 1;
        snapshot.updated_at = updated_at;
        let snapshot = snapshot.clone();
        self.replace_state(next)?;

        Ok(UpdateWorkRes { snapshot })
    }

    fn create_contract_draft(
        &self,
        req: CreateContractDraftReq,
    ) -> Result<CreateContractDraftRes, StoreError> {
        let mut next = self.cloned_state();

        if !next.companies.contains_key(req.company_id.as_str()) {
            return Err(conflict(
                "create_contract_draft requires a registered company",
            ));
        }

        let existing_company_contracts = next
            .contract_history
            .iter()
            .filter(|contract| contract.company_id == req.company_id)
            .cloned()
            .collect::<Vec<_>>();
        let revision = next
            .contract_history
            .iter()
            .filter(|contract| contract.company_id == req.company_id)
            .map(|contract| contract.revision)
            .max()
            .unwrap_or(0)
            + 1;
        let contract_set_id = existing_company_contracts
            .iter()
            .max_by_key(|contract| contract.revision)
            .map(|contract| contract.contract_set_id.clone())
            .unwrap_or_else(|| {
                next.next_contract_seq += 1;
                ContractSetId::from(sequenced_id(next.next_contract_seq))
            });
        next.contract_history.push(ContractSet {
            contract_set_id,
            company_id: req.company_id,
            revision,
            name: req.name,
            status: ContractSetStatus::Draft,
            rules: req.rules,
        });
        self.replace_state(next)?;

        Ok(CreateContractDraftRes { revision })
    }

    fn activate_contract(
        &self,
        req: ActivateContractReq,
    ) -> Result<ActivateContractRes, StoreError> {
        let mut next = self.cloned_state();

        if !next.companies.contains_key(req.company_id.as_str()) {
            return Err(conflict("activate_contract requires a registered company"));
        }

        let mut activated = None;
        for contract in &mut next.contract_history {
            if contract.company_id != req.company_id {
                continue;
            }

            if contract.revision == req.revision {
                contract.status = ContractSetStatus::Active;
                activated = Some(contract.clone());
            } else if contract.status == ContractSetStatus::Active {
                contract.status = ContractSetStatus::Retired;
            }
        }

        let activated = activated.ok_or_else(|| StoreError {
            kind: StoreErrorKind::NotFound,
            message: format!(
                "memory store capability `activate_contract` could not find revision {}",
                req.revision
            ),
        })?;

        next.contract = activated;
        self.replace_state(next)?;

        Ok(ActivateContractRes {
            revision: req.revision,
        })
    }

    fn claim_lease(&self, req: ClaimLeaseReq) -> Result<ClaimLeaseRes, StoreError> {
        MemoryStore::claim_lease(self, req)
    }

    fn read_board(&self) -> BoardReadModel {
        query::read_board(self)
    }

    fn read_companies(&self) -> CompanyReadModel {
        query::read_companies(self)
    }

    fn read_work(&self, work_id: Option<&WorkId>) -> Result<WorkReadModel, StoreError> {
        query::read_work(self, work_id)
    }

    fn read_agents(&self) -> AgentReadModel {
        query::read_agents(self)
    }

    fn read_activity(&self) -> ActivityReadModel {
        query::read_activity(self)
    }

    fn read_run(&self, run_id: &RunId) -> Result<RunReadModel, StoreError> {
        query::read_run(self, run_id)
    }

    fn read_contracts(&self) -> ContractsReadModel {
        query::read_contracts(self)
    }

    fn load_context(&self, work_id: &WorkId) -> Result<WorkContext, StoreError> {
        let state = self.state.borrow();
        let snapshot = state
            .snapshots
            .get(work_id.as_str())
            .cloned()
            .ok_or_else(|| not_found("load_context", work_id))?;
        let lease = snapshot
            .active_lease_id
            .as_ref()
            .and_then(|lease_id| state.leases.get(lease_id.as_str()))
            .cloned()
            .filter(|lease| lease.released_at.is_none());
        let pending_wake = state.pending_wakes.get(work_id.as_str()).cloned();
        let contract = contract_for_snapshot(&state, &snapshot)
            .ok_or_else(|| conflict("load_context requires the work's pinned contract revision"))?;

        Ok(WorkContext {
            snapshot,
            lease,
            pending_wake,
            contract,
        })
    }

    fn list_work_snapshots(&self) -> Result<Vec<WorkSnapshot>, StoreError> {
        let mut snapshots = self
            .state
            .borrow()
            .snapshots
            .values()
            .cloned()
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.work_id.cmp(&right.work_id));
        Ok(snapshots)
    }

    fn load_transition_records(
        &self,
        work_id: &WorkId,
    ) -> Result<Vec<TransitionRecord>, StoreError> {
        let mut records = self
            .state
            .borrow()
            .transition_records
            .iter()
            .filter(|record| record.work_id == *work_id)
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.expected_rev
                .cmp(&right.expected_rev)
                .then_with(|| left.happened_at.cmp(&right.happened_at))
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        Ok(records)
    }

    fn merge_wake(&self, req: MergeWakeReq) -> Result<PendingWake, StoreError> {
        let mut next = self.cloned_state();
        let snapshot = next
            .snapshots
            .get(req.work_id.as_str())
            .cloned()
            .ok_or_else(|| not_found("merge_wake", &req.work_id))?;

        let existing = next.pending_wakes.get(req.work_id.as_str()).cloned();
        let merged_at = next.next_timestamp();
        let merged = kernel::merge_wake(
            existing.as_ref(),
            &req.reason,
            &req.obligations,
            merged_at,
            req.work_id.clone(),
        );

        next.pending_wakes
            .insert(req.work_id.0.clone(), merged.clone());
        let source = req.source.clone();
        next.push_comment(
            snapshot.company_id.clone(),
            req.work_id.clone(),
            req.actor_kind,
            req.actor_id,
            Some(source),
            wake_comment_body(&req.source, &req.reason, &req.obligations, merged.count),
        );
        next.ensure_wake_runnable_run(&snapshot, merged_at);
        self.replace_state(next)?;

        Ok(merged)
    }

    fn reap_timed_out_runs(&self, timeout: Duration) -> Result<Vec<ReapedRun>, StoreError> {
        let mut next = self.cloned_state();
        let reaped_at = next.next_timestamp();
        let stale_run_ids = next
            .runs
            .values()
            .filter(|run| {
                run.status == RunStatus::Running
                    && matches!(
                        reaped_at.duration_since(run.updated_at),
                        Ok(elapsed) if elapsed >= timeout
                    )
            })
            .map(|run| run.run_id.clone())
            .collect::<Vec<_>>();

        let mut reaped_runs = Vec::with_capacity(stale_run_ids.len());
        for run_id in stale_run_ids {
            let Some(stale_run) = next.runs.get(run_id.as_str()).cloned() else {
                continue;
            };
            let Some(run) = next.runs.get_mut(run_id.as_str()) else {
                continue;
            };
            run.status = RunStatus::TimedOut;
            run.updated_at = reaped_at;
            next.activity_events.push(activity_entry_from_run(run));

            let Some(snapshot) = next.snapshots.get(stale_run.work_id.as_str()).cloned() else {
                continue;
            };
            let released_lease_id =
                next.find_open_lease_for_run(&stale_run.work_id, &stale_run.run_id);
            if let Some(lease_id) = released_lease_id.as_ref() {
                let req = timeout_requeue_commit_req(&snapshot, &stale_run, lease_id, reaped_at);
                commit::apply_commit_decision_to_state(&mut next, req)?;
            }
            let follow_up_run_id = next
                .pending_wakes
                .contains_key(stale_run.work_id.as_str())
                .then(|| next.snapshots.get(stale_run.work_id.as_str()).cloned())
                .flatten()
                .and_then(|snapshot| next.ensure_wake_runnable_run(&snapshot, reaped_at));
            next.push_comment(
                stale_run.company_id.clone(),
                stale_run.work_id.clone(),
                ActorKind::System,
                ActorId::from("system"),
                Some("scheduler".to_owned()),
                reaper_comment_body(
                    &stale_run.run_id,
                    released_lease_id.as_ref(),
                    follow_up_run_id.as_ref(),
                ),
            );

            reaped_runs.push(ReapedRun {
                run_id: stale_run.run_id,
                work_id: stale_run.work_id,
                released_lease_id,
                follow_up_run_id,
            });
        }

        self.replace_state(next)?;
        Ok(reaped_runs)
    }

    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError> {
        Ok(self.state.borrow().sessions.get(&session_key(key)).cloned())
    }

    fn load_queued_runs(&self) -> Result<Vec<QueuedRunCandidate>, StoreError> {
        let state = self.state.borrow();
        Ok(state
            .runs
            .values()
            .filter(|run| run.status == RunStatus::Queued)
            .map(|run| QueuedRunCandidate {
                run_id: run.run_id.clone(),
                agent_status: state
                    .agents
                    .get(run.agent_id.as_str())
                    .map(|agent| agent.status),
                budget_blocked: company_budget_hard_stopped(&state, &run.company_id),
                created_at: run.created_at,
            })
            .collect())
    }

    fn load_runtime_turn(&self, run_id: &RunId) -> Result<RuntimeTurnContext, StoreError> {
        let state = self.state.borrow();
        let run = state
            .runs
            .get(run_id.as_str())
            .cloned()
            .ok_or_else(|| run_not_found("load_runtime_turn", run_id))?;

        if !run.status.is_runnable() {
            return Err(conflict(
                "load_runtime_turn requires a queued or running run",
            ));
        }

        let agent = state
            .agents
            .get(run.agent_id.as_str())
            .ok_or_else(|| conflict("load_runtime_turn requires a registered agent"))?;
        if agent.status != AgentStatus::Active {
            return Err(conflict(
                "load_runtime_turn requires an active agent for queued or running run",
            ));
        }

        let snapshot = state
            .snapshots
            .get(run.work_id.as_str())
            .cloned()
            .ok_or_else(|| not_found("load_runtime_turn snapshot", &run.work_id))?;
        let pending_wake = state.pending_wakes.get(run.work_id.as_str()).cloned();
        let contract = contract_for_snapshot(&state, &snapshot).ok_or_else(|| {
            conflict("load_runtime_turn requires the run work's pinned contract revision")
        })?;

        Ok(RuntimeTurnContext {
            run_id: run.run_id,
            agent_id: run.agent_id,
            snapshot,
            pending_wake,
            contract,
        })
    }

    fn load_agent_facts(&self, agent_id: &AgentId) -> Result<Option<AgentFacts>, StoreError> {
        let state = self.state.borrow();
        Ok(state.agents.get(agent_id.as_str()).map(|agent| AgentFacts {
            company_id: agent.company_id.clone(),
            status: agent.status,
        }))
    }

    fn mark_run_running(&self, run_id: &RunId) -> Result<(), StoreError> {
        let mut next = self.cloned_state();
        let updated_at = next.next_timestamp();
        let run = next
            .runs
            .get_mut(run_id.as_str())
            .ok_or_else(|| run_not_found("mark_run_running", run_id))?;

        if !run.status.is_runnable() {
            return Err(conflict(
                "mark_run_running requires a queued or running run",
            ));
        }

        let agent = next
            .agents
            .get(run.agent_id.as_str())
            .ok_or_else(|| conflict("mark_run_running requires a registered agent"))?;
        if agent.status != AgentStatus::Active {
            return Err(conflict("mark_run_running requires an active agent"));
        }

        let should_record_activity = run.status != RunStatus::Running;
        run.status = RunStatus::Running;
        run.updated_at = updated_at;
        if should_record_activity {
            next.activity_events.push(activity_entry_from_run(run));
        }
        self.replace_state(next)?;
        Ok(())
    }

    fn mark_run_completed(&self, run_id: &RunId) -> Result<(), StoreError> {
        let mut next = self.cloned_state();
        let updated_at = next.next_timestamp();
        let run = next
            .runs
            .get_mut(run_id.as_str())
            .ok_or_else(|| run_not_found("mark_run_completed", run_id))?;

        if !run.status.is_runnable() {
            return Err(conflict(
                "mark_run_completed requires a queued or running run",
            ));
        }

        run.status = RunStatus::Completed;
        run.updated_at = updated_at;
        next.activity_events
            .push(activity_entry_from_completed_run(run));
        self.replace_state(next)?;
        Ok(())
    }

    fn mark_run_failed(&self, run_id: &RunId, reason: &str) -> Result<(), StoreError> {
        let mut next = self.cloned_state();
        let updated_at = next.next_timestamp();
        let run = next
            .runs
            .get_mut(run_id.as_str())
            .ok_or_else(|| run_not_found("mark_run_failed", run_id))?;

        if !run.status.is_runnable() {
            return Err(conflict("mark_run_failed requires a queued or running run"));
        }

        run.status = RunStatus::Failed;
        run.updated_at = updated_at;
        next.activity_events
            .push(activity_entry_from_failed_run(run, reason));
        self.replace_state(next)?;
        Ok(())
    }

    fn save_session(&self, session: &TaskSession) -> Result<(), StoreError> {
        let mut next = self.cloned_state();
        next.sessions.insert(
            session_key_parts(&session.agent_id, &session.work_id),
            session.clone(),
        );
        self.replace_state(next)?;
        Ok(())
    }

    fn record_consumption(&self, req: RecordConsumptionReq) -> Result<(), StoreError> {
        let mut next = self.cloned_state();
        let run = next
            .runs
            .get(req.run_id.as_str())
            .ok_or_else(|| run_not_found("record_consumption", &req.run_id))?;
        let run_company_id = run.company_id.clone();
        let run_agent_id = run.agent_id.clone();

        if run_company_id != req.company_id || run_agent_id != req.agent_id {
            return Err(conflict(
                "record_consumption rejects company/run or agent/run boundary violations",
            ));
        }

        next.next_consumption_seq += 1;
        let created_at = next.next_timestamp();
        let estimated_cost_cents = req.usage.estimated_cost_cents.unwrap_or(0);
        next.consumption_events.push(PersistedConsumptionEvent {
            event_id: format!("consumption-{}", next.next_consumption_seq),
            company_id: req.company_id,
            agent_id: req.agent_id,
            run_id: req.run_id,
            billing_kind: req.billing_kind,
            usage: req.usage,
            created_at,
        });
        if let Some(company) = next.companies.get_mut(run_company_id.as_str()) {
            company.recorded_estimated_cost_cents += estimated_cost_cents;
        }
        self.replace_state(next)?;
        Ok(())
    }

    fn commit_decision(&self, req: CommitDecisionReq) -> Result<CommitDecisionRes, StoreError> {
        let mut next = self.cloned_state();
        let authoritative = commit::load_commit_authoritative_state(&next, &req)?;
        let prepared = commit::prepare_commit_decision(&mut next, req, authoritative);
        let result = commit::execute_commit_decision_apply(&mut next, prepared)?;
        self.replace_state(next)?;
        Ok(result)
    }
}

impl MemoryState {
    fn demo() -> Self {
        let contract_history = demo_contract_history();
        let contract = contract_history
            .iter()
            .find(|contract| contract.status == ContractSetStatus::Active)
            .cloned()
            .expect("demo contract history should contain one active revision");
        let company_id = CompanyId::from(DEMO_COMPANY_ID);
        let contract_set_id = ContractSetId::from(DEMO_CONTRACT_SET_ID);
        let mut agents = BTreeMap::new();
        agents.insert(
            DEMO_AGENT_ID.to_owned(),
            RegisteredAgent {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                company_id: company_id.clone(),
                name: "AxiomNexus Operator".to_owned(),
                role: "implementer".to_owned(),
                status: AgentStatus::Active,
            },
        );
        agents.insert(
            DEMO_PAUSED_AGENT_ID.to_owned(),
            RegisteredAgent {
                agent_id: AgentId::from(DEMO_PAUSED_AGENT_ID),
                company_id: company_id.clone(),
                name: "Paused Agent".to_owned(),
                role: "reviewer".to_owned(),
                status: AgentStatus::Paused,
            },
        );
        agents.insert(
            DEMO_TERMINATED_AGENT_ID.to_owned(),
            RegisteredAgent {
                agent_id: AgentId::from(DEMO_TERMINATED_AGENT_ID),
                company_id: company_id.clone(),
                name: "Terminated Agent".to_owned(),
                role: "archived".to_owned(),
                status: AgentStatus::Terminated,
            },
        );
        agents.insert(
            DEMO_FOREIGN_AGENT_ID.to_owned(),
            RegisteredAgent {
                agent_id: AgentId::from(DEMO_FOREIGN_AGENT_ID),
                company_id: CompanyId::from(DEMO_FOREIGN_COMPANY_ID),
                name: "Foreign Agent".to_owned(),
                role: "external".to_owned(),
                status: AgentStatus::Active,
            },
        );

        let todo_snapshot = WorkSnapshot {
            work_id: WorkId::from(DEMO_TODO_WORK_ID),
            company_id: company_id.clone(),
            parent_id: None,
            kind: WorkKind::Task,
            title: "Todo work".to_owned(),
            body: String::new(),
            status: WorkStatus::Todo,
            priority: Priority::Medium,
            assignee_agent_id: None,
            active_lease_id: None,
            rev: 0,
            contract_set_id: contract_set_id.clone(),
            contract_rev: contract.revision,
            created_at: SystemTime::UNIX_EPOCH,
            updated_at: SystemTime::UNIX_EPOCH,
        };

        let doing_snapshot = WorkSnapshot {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            company_id: company_id.clone(),
            parent_id: None,
            kind: WorkKind::Task,
            title: "Doing work".to_owned(),
            body: String::new(),
            status: WorkStatus::Doing,
            priority: Priority::High,
            assignee_agent_id: Some(AgentId::from(DEMO_AGENT_ID)),
            active_lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
            rev: 1,
            contract_set_id,
            contract_rev: contract.revision,
            created_at: SystemTime::UNIX_EPOCH,
            updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        };

        let lease = WorkLease {
            lease_id: LeaseId::from(DEMO_LEASE_ID),
            company_id,
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            run_id: Some(RunId::from("run-1")),
            acquired_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            expires_at: None,
            released_at: None,
            release_reason: None,
        };
        let mut runs = BTreeMap::new();
        runs.insert(
            "run-1".to_owned(),
            RegisteredRun {
                run_id: RunId::from("run-1"),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                status: RunStatus::Running,
                created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
                updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            },
        );

        let mut snapshots = BTreeMap::new();
        snapshots.insert(todo_snapshot.work_id.0.clone(), todo_snapshot);
        snapshots.insert(doing_snapshot.work_id.0.clone(), doing_snapshot);

        let mut leases = BTreeMap::new();
        leases.insert(lease.lease_id.0.clone(), lease);

        Self {
            companies: default_companies(),
            agents,
            runs,
            snapshots,
            leases,
            pending_wakes: BTreeMap::new(),
            sessions: BTreeMap::new(),
            comments: Vec::new(),
            consumption_events: Vec::new(),
            transition_records: Vec::new(),
            activity_events: Vec::new(),
            contract,
            contract_history,
            next_comment_seq: 0,
            next_consumption_seq: 0,
            next_agent_seq: 6,
            next_company_seq: 3,
            next_contract_seq: 14,
            next_work_seq: 12,
            next_run_seq: 1,
            next_lease_seq: 32,
            tick: 10,
        }
    }

    fn next_timestamp(&mut self) -> SystemTime {
        self.tick += 1;
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.tick)
    }

    fn push_comment(
        &mut self,
        company_id: CompanyId,
        work_id: WorkId,
        author_kind: ActorKind,
        author_id: ActorId,
        source: Option<String>,
        body: String,
    ) -> String {
        self.next_comment_seq += 1;
        let comment_id = format!("comment-{}", self.next_comment_seq);
        let created_at = self.next_timestamp();
        let comment = PersistedComment {
            comment_id: comment_id.clone(),
            company_id,
            work_id,
            author_kind,
            author_id,
            source,
            body,
            created_at,
        };
        self.activity_events
            .push(activity_entry_from_comment(&comment));
        self.comments.push(comment);
        comment_id
    }

    fn ensure_runnable_run(
        &mut self,
        snapshot: &WorkSnapshot,
        agent_id: &AgentId,
        updated_at: SystemTime,
    ) -> RunId {
        if let Some(run) = self
            .runs
            .values_mut()
            .find(|run| run.work_id == snapshot.work_id && run.status.is_runnable())
        {
            let previous_status = run.status;
            run.agent_id = agent_id.clone();
            run.status = RunStatus::Running;
            run.updated_at = updated_at;
            if previous_status != RunStatus::Running {
                self.activity_events.push(activity_entry_from_run(run));
            }
            return run.run_id.clone();
        }

        self.next_run_seq += 1;
        let run_id = RunId::from(format!("run-{}", self.next_run_seq));
        let run = RegisteredRun {
            run_id: run_id.clone(),
            company_id: snapshot.company_id.clone(),
            agent_id: agent_id.clone(),
            work_id: snapshot.work_id.clone(),
            status: RunStatus::Running,
            created_at: updated_at,
            updated_at,
        };
        self.activity_events.push(activity_entry_from_run(&run));
        self.runs.insert(run_id.0.clone(), run);
        run_id
    }

    fn ensure_wake_runnable_run(
        &mut self,
        snapshot: &WorkSnapshot,
        updated_at: SystemTime,
    ) -> Option<RunId> {
        let has_open_lease = snapshot
            .active_lease_id
            .as_ref()
            .and_then(|lease_id| self.leases.get(lease_id.as_str()))
            .is_some_and(|lease| lease.released_at.is_none());
        let existing_run = self
            .runs
            .values_mut()
            .find(|run| run.work_id == snapshot.work_id && run.status.is_runnable())
            .map(|run| run.run_id.clone());
        let runnable_agent = self.pick_runnable_agent(snapshot);

        match kernel::wake_run_plan(
            has_open_lease,
            existing_run.is_some(),
            runnable_agent.is_some(),
        ) {
            crate::kernel::WakeRunPlan::SkipBecauseOpenLease
            | crate::kernel::WakeRunPlan::SkipBecauseNoRunnableAgent => None,
            crate::kernel::WakeRunPlan::RefreshExistingRun => {
                let run_id = existing_run?;
                if let Some(run) = self.runs.get_mut(run_id.as_str()) {
                    run.updated_at = updated_at;
                }
                Some(run_id)
            }
            crate::kernel::WakeRunPlan::QueueNewRun => {
                let agent_id = runnable_agent?;
                self.next_run_seq += 1;
                let run_id = RunId::from(format!("run-{}", self.next_run_seq));
                let run = RegisteredRun {
                    run_id: run_id.clone(),
                    company_id: snapshot.company_id.clone(),
                    agent_id,
                    work_id: snapshot.work_id.clone(),
                    status: RunStatus::Queued,
                    created_at: updated_at,
                    updated_at,
                };
                self.activity_events.push(activity_entry_from_run(&run));
                self.runs.insert(run_id.0.clone(), run);
                Some(run_id)
            }
        }
    }

    fn find_open_lease_for_run(&self, work_id: &WorkId, run_id: &RunId) -> Option<LeaseId> {
        let lease_id = self
            .snapshots
            .get(work_id.as_str())
            .and_then(|snapshot| snapshot.active_lease_id.clone())
            .filter(|lease_id| {
                self.leases.get(lease_id.as_str()).is_some_and(|lease| {
                    lease.released_at.is_none() && lease.run_id.as_ref() == Some(run_id)
                })
            })
            .or_else(|| {
                self.leases
                    .values()
                    .find(|lease| {
                        lease.work_id == *work_id
                            && lease.released_at.is_none()
                            && lease.run_id.as_ref() == Some(run_id)
                    })
                    .map(|lease| lease.lease_id.clone())
            });

        lease_id
    }

    fn pick_runnable_agent(&self, snapshot: &WorkSnapshot) -> Option<AgentId> {
        if let Some(agent_id) = snapshot.assignee_agent_id.as_ref() {
            let registered = self.agents.get(agent_id.as_str())?;
            if registered.company_id == snapshot.company_id
                && registered.status == AgentStatus::Active
            {
                return Some(agent_id.clone());
            }
        }

        self.agents.values().find_map(|agent| {
            (agent.company_id == snapshot.company_id && agent.status == AgentStatus::Active)
                .then(|| agent.agent_id.clone())
        })
    }
}

fn demo_contract_history() -> Vec<ContractSet> {
    let template = serde_json::from_str::<ContractTemplate>(include_str!(
        "../../../samples/company-rust-contract.example.json"
    ))
    .expect("canonical company contract sample should parse");

    vec![
        ContractSet {
            contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            revision: template.revision.saturating_sub(1),
            name: format!("{} v{}", template.name, template.revision.saturating_sub(1)),
            status: ContractSetStatus::Retired,
            rules: template.rules.clone(),
        },
        ContractSet {
            contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            revision: template.revision,
            name: template.name.clone(),
            status: ContractSetStatus::Active,
            rules: template.rules.clone(),
        },
        ContractSet {
            contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            revision: template.revision + 1,
            name: format!("{} draft", template.name),
            status: ContractSetStatus::Draft,
            rules: template.rules,
        },
    ]
}

fn work_key(work_id: &WorkId) -> String {
    work_id.as_str().to_owned()
}

fn session_key(key: &SessionKey) -> String {
    session_key_parts(&key.agent_id, &key.work_id)
}

fn default_companies() -> BTreeMap<String, CompanyProfile> {
    let mut companies = BTreeMap::new();
    companies.insert(
        DEMO_COMPANY_ID.to_owned(),
        CompanyProfile {
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            name: "AxiomNexus Demo".to_owned(),
            description: "demo company".to_owned(),
            runtime_hard_stop_cents: None,
            recorded_estimated_cost_cents: 0,
        },
    );
    companies.insert(
        DEMO_FOREIGN_COMPANY_ID.to_owned(),
        CompanyProfile {
            company_id: CompanyId::from(DEMO_FOREIGN_COMPANY_ID),
            name: "Foreign Demo".to_owned(),
            description: "foreign boundary demo".to_owned(),
            runtime_hard_stop_cents: None,
            recorded_estimated_cost_cents: 0,
        },
    );
    companies
}

fn session_key_parts(agent_id: &AgentId, work_id: &WorkId) -> String {
    format!("{}:{}", agent_id.as_str(), work_id.as_str())
}

fn work_summary_for(state: &MemoryState, work_id: &WorkId) -> Option<WorkSummary> {
    let audit_entries = work_activity_entries(state, work_id);
    state.snapshots.get(work_id.as_str()).map(|snapshot| {
        work_summary_with_comments(
            snapshot,
            state.pending_wakes.get(work_id.as_str()),
            &state.contract_history,
            &state.comments,
            &audit_entries,
        )
    })
}

fn active_contract_for_company(state: &MemoryState, company_id: &CompanyId) -> Option<ContractSet> {
    state
        .contract_history
        .iter()
        .filter(|contract| {
            contract.company_id == *company_id && contract.status == ContractSetStatus::Active
        })
        .max_by_key(|contract| contract.revision)
        .cloned()
}

fn contract_for_snapshot(state: &MemoryState, snapshot: &WorkSnapshot) -> Option<ContractSet> {
    state
        .contract_history
        .iter()
        .find(|contract| {
            contract.company_id == snapshot.company_id
                && contract.contract_set_id == snapshot.contract_set_id
                && contract.revision == snapshot.contract_rev
        })
        .cloned()
}

fn work_summary(
    snapshot: &WorkSnapshot,
    pending_wake: Option<&PendingWake>,
    contract_history: &[ContractSet],
) -> WorkSummary {
    let (contract_name, contract_status) = contract_summary(snapshot, contract_history);
    WorkSummary {
        work_id: snapshot.work_id.as_str().to_owned(),
        parent_id: snapshot
            .parent_id
            .as_ref()
            .map(|parent_id| parent_id.as_str().to_owned()),
        kind: snapshot.kind,
        title: snapshot.title.clone(),
        body: snapshot.body.clone(),
        status: snapshot.status,
        rev: snapshot.rev,
        active_lease_id: snapshot
            .active_lease_id
            .as_ref()
            .map(|lease_id| lease_id.as_str().to_owned()),
        contract_set_id: snapshot.contract_set_id.as_str().to_owned(),
        contract_rev: snapshot.contract_rev,
        contract_name,
        contract_status,
        pending_obligations: pending_wake
            .map(|wake| wake.obligation_json.iter().cloned().collect())
            .unwrap_or_default(),
        comments: Vec::new(),
        audit_entries: Vec::new(),
    }
}

fn work_summary_with_comments(
    snapshot: &WorkSnapshot,
    pending_wake: Option<&PendingWake>,
    contract_history: &[ContractSet],
    comments: &[PersistedComment],
    audit_entries: &[ActivityEntryView],
) -> WorkSummary {
    let mut summary = work_summary(snapshot, pending_wake, contract_history);
    summary.comments = comments
        .iter()
        .filter(|comment| comment.work_id == snapshot.work_id)
        .map(|comment| WorkCommentView {
            author_kind: comment.author_kind,
            author_id: comment.author_id.as_str().to_owned(),
            source: comment.source.clone(),
            body: comment.body.clone(),
        })
        .collect();
    summary.audit_entries = audit_entries
        .iter()
        .filter(|entry| entry.work_id == snapshot.work_id.as_str())
        .cloned()
        .collect();
    summary
}

fn contract_summary(
    snapshot: &WorkSnapshot,
    contract_history: &[ContractSet],
) -> (Option<String>, Option<ContractSetStatus>) {
    contract_history
        .iter()
        .find(|contract| {
            contract.contract_set_id == snapshot.contract_set_id
                && contract.revision == snapshot.contract_rev
        })
        .map(|contract| (Some(contract.name.clone()), Some(contract.status)))
        .unwrap_or((None, None))
}

fn activity_entries(state: &MemoryState) -> Vec<ActivityEntryView> {
    activity_views(state.activity_events.iter().cloned(), Some(20))
}

fn work_activity_entries(state: &MemoryState, work_id: &WorkId) -> Vec<ActivityEntryView> {
    activity_views(
        state
            .activity_events
            .iter()
            .filter(|entry| entry.view.work_id == work_id.as_str())
            .cloned(),
        Some(20),
    )
}

fn would_create_work_cycle(state: &MemoryState, work_id: &WorkId, parent_id: &WorkId) -> bool {
    let mut current = Some(parent_id.clone());
    while let Some(candidate_id) = current {
        if &candidate_id == work_id {
            return true;
        }
        current = state
            .snapshots
            .get(candidate_id.as_str())
            .and_then(|snapshot| snapshot.parent_id.clone());
    }
    false
}

fn activity_views(
    entries: impl Iterator<Item = TimedActivityEntry>,
    limit: Option<usize>,
) -> Vec<ActivityEntryView> {
    let mut entries = entries.collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .sort_key
            .cmp(&left.sort_key)
            .then_with(|| right.priority.cmp(&left.priority))
    });
    let entries = entries.into_iter();
    match limit {
        Some(limit) => entries.take(limit).map(|entry| entry.view).collect(),
        None => entries.map(|entry| entry.view).collect(),
    }
}

fn consumption_summary(events: &[PersistedConsumptionEvent]) -> ConsumptionSummaryView {
    let mut summary = ConsumptionSummaryView {
        total_turns: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_run_seconds: 0,
        total_estimated_cost_cents: 0,
    };

    for event in events {
        summary.total_turns += 1;
        summary.total_input_tokens += event.usage.input_tokens;
        summary.total_output_tokens += event.usage.output_tokens;
        summary.total_run_seconds += event.usage.run_seconds;
        summary.total_estimated_cost_cents += event.usage.estimated_cost_cents.unwrap_or(0);
    }

    summary
}

fn company_budget_hard_stopped(state: &MemoryState, company_id: &CompanyId) -> bool {
    let Some(company) = state.companies.get(company_id.as_str()) else {
        return false;
    };
    let Some(limit) = company.runtime_hard_stop_cents else {
        return false;
    };

    company.recorded_estimated_cost_cents >= limit
}

fn agent_consumption_summaries(state: &MemoryState) -> Vec<AgentConsumptionSummaryView> {
    state
        .agents
        .values()
        .map(|agent| {
            let mut summary = AgentConsumptionSummaryView {
                agent_id: agent.agent_id.as_str().to_owned(),
                total_turns: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_run_seconds: 0,
                total_estimated_cost_cents: 0,
            };

            for event in state
                .consumption_events
                .iter()
                .filter(|event| event.agent_id == agent.agent_id)
            {
                summary.total_turns += 1;
                summary.total_input_tokens += event.usage.input_tokens;
                summary.total_output_tokens += event.usage.output_tokens;
                summary.total_run_seconds += event.usage.run_seconds;
                summary.total_estimated_cost_cents += event.usage.estimated_cost_cents.unwrap_or(0);
            }

            summary
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimedActivityEntry {
    sort_key: SystemTime,
    priority: u8,
    view: ActivityEntryView,
}

struct CommitAuthoritativeState {
    snapshot: WorkSnapshot,
    live_lease: Option<WorkLease>,
}

struct PreparedCommitDecision {
    req: CommitDecisionReq,
    activity_event: TimedActivityEntry,
}

fn activity_entry_from_transition(record: &TransitionRecord) -> TimedActivityEntry {
    TimedActivityEntry {
        sort_key: record.happened_at,
        priority: 3,
        view: store_support::transition_activity_entry_view(record),
    }
}

fn activity_entry_from_comment(comment: &PersistedComment) -> TimedActivityEntry {
    TimedActivityEntry {
        sort_key: comment.created_at,
        priority: 2,
        view: ActivityEntryView {
            event_kind: "comment".to_owned(),
            work_id: comment.work_id.as_str().to_owned(),
            summary: comment.body.clone(),
            actor_kind: Some(comment.author_kind),
            actor_id: Some(comment.author_id.as_str().to_owned()),
            source: comment.source.clone(),
            before_status: None,
            after_status: None,
            outcome: None,
            evidence_summary: None,
        },
    }
}

fn activity_entry_from_run(run: &RegisteredRun) -> TimedActivityEntry {
    TimedActivityEntry {
        sort_key: run.updated_at,
        priority: 1,
        view: ActivityEntryView {
            event_kind: "run".to_owned(),
            work_id: run.work_id.as_str().to_owned(),
            summary: format!("run {} {}", run.run_id, run_status_label(run.status)),
            actor_kind: None,
            actor_id: None,
            source: None,
            before_status: None,
            after_status: None,
            outcome: None,
            evidence_summary: None,
        },
    }
}

fn activity_entry_from_failed_run(run: &RegisteredRun, reason: &str) -> TimedActivityEntry {
    TimedActivityEntry {
        sort_key: run.updated_at,
        priority: 1,
        view: ActivityEntryView {
            event_kind: "run".to_owned(),
            work_id: run.work_id.as_str().to_owned(),
            summary: format!("run {} failed: {reason}", run.run_id),
            actor_kind: None,
            actor_id: None,
            source: Some("runtime".to_owned()),
            before_status: None,
            after_status: None,
            outcome: Some("failed".to_owned()),
            evidence_summary: None,
        },
    }
}

fn activity_entry_from_completed_run(run: &RegisteredRun) -> TimedActivityEntry {
    TimedActivityEntry {
        sort_key: run.updated_at,
        priority: 1,
        view: ActivityEntryView {
            event_kind: "run".to_owned(),
            work_id: run.work_id.as_str().to_owned(),
            summary: format!("run {} completed", run.run_id),
            actor_kind: None,
            actor_id: None,
            source: Some("runtime".to_owned()),
            before_status: None,
            after_status: None,
            outcome: Some("completed".to_owned()),
            evidence_summary: None,
        },
    }
}

fn timeout_requeue_commit_req(
    snapshot: &WorkSnapshot,
    stale_run: &RegisteredRun,
    lease_id: &LeaseId,
    reaped_at: SystemTime,
) -> CommitDecisionReq {
    store_support::timeout_requeue_commit_req(
        snapshot,
        stale_run.run_id.as_str(),
        lease_id,
        reaped_at,
    )
}

fn reaper_comment_body(
    run_id: &RunId,
    released_lease_id: Option<&LeaseId>,
    follow_up_run_id: Option<&RunId>,
) -> String {
    let lease = released_lease_id
        .map(|lease_id| format!(" lease {} expired;", lease_id))
        .unwrap_or_default();
    let follow_up = follow_up_run_id
        .map(|queued_run_id| format!(" follow-up {} queued.", queued_run_id))
        .unwrap_or_else(|| " no follow-up queued.".to_owned());

    format!(
        "system reaper timed out run {}.{}{}",
        run_id, lease, follow_up
    )
}

fn wake_comment_body(
    source: &str,
    reason: &str,
    obligations: &[String],
    merged_count: u32,
) -> String {
    let obligations = if obligations.is_empty() {
        "none".to_owned()
    } else {
        obligations.join(", ")
    };

    format!(
        "{source} wake merged: reason={reason}; count={merged_count}; obligations={obligations}"
    )
}

fn decision_outcome_label(outcome: crate::model::DecisionOutcome) -> &'static str {
    match outcome {
        crate::model::DecisionOutcome::Accepted => "accepted",
        crate::model::DecisionOutcome::Rejected => "rejected",
        crate::model::DecisionOutcome::Conflict => "conflict",
        crate::model::DecisionOutcome::OverrideAccepted => "override_accepted",
    }
}

fn transition_kind_label(kind: TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Queue => "queue",
        TransitionKind::Claim => "claim",
        TransitionKind::ProposeProgress => "propose_progress",
        TransitionKind::Complete => "complete",
        TransitionKind::Block => "block",
        TransitionKind::Reopen => "reopen",
        TransitionKind::Cancel => "cancel",
        TransitionKind::OverrideComplete => "override_complete",
        TransitionKind::TimeoutRequeue => "timeout_requeue",
    }
}

fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        RunStatus::TimedOut => "timed_out",
    }
}

fn not_found(capability: &str, entity_id: &impl std::fmt::Display) -> StoreError {
    StoreError {
        kind: StoreErrorKind::NotFound,
        message: format!(
            "memory store capability `{capability}` could not find entity {}",
            entity_id
        ),
    }
}

fn run_not_found(capability: &str, run_id: &RunId) -> StoreError {
    StoreError {
        kind: StoreErrorKind::NotFound,
        message: format!(
            "memory store capability `{capability}` could not find run {}",
            run_id
        ),
    }
}

fn conflict(message: &str) -> StoreError {
    StoreError {
        kind: StoreErrorKind::Conflict,
        message: message.to_owned(),
    }
}

fn sequenced_id(sequence: u64) -> String {
    format!("00000000-0000-4000-8000-{sequence:012x}")
}

fn release_reason_for(kind: TransitionKind) -> LeaseReleaseReason {
    match kind {
        TransitionKind::Complete => LeaseReleaseReason::Completed,
        TransitionKind::Block => LeaseReleaseReason::Blocked,
        TransitionKind::Cancel => LeaseReleaseReason::Cancelled,
        TransitionKind::OverrideComplete => LeaseReleaseReason::Overridden,
        TransitionKind::TimeoutRequeue => LeaseReleaseReason::Expired,
        TransitionKind::Queue
        | TransitionKind::Claim
        | TransitionKind::ProposeProgress
        | TransitionKind::Reopen => LeaseReleaseReason::Conflict,
    }
}

fn ensure_commit_preconditions(
    state: &MemoryState,
    req: &CommitDecisionReq,
) -> Result<(), StoreError> {
    let snapshot = state
        .snapshots
        .get(req.context.record.work_id.as_str())
        .ok_or_else(|| not_found("commit_decision", &req.context.record.work_id))?;

    if snapshot.rev != req.context.record.expected_rev {
        return Err(conflict(
            "commit_decision expected_rev does not match authoritative snapshot",
        ));
    }

    match req.effects.lease {
        LeaseEffect::Acquire => {
            if req.context.record.lease_id.is_none() {
                return Err(conflict(
                    "commit_decision acquire requires a lease_id on the record",
                ));
            }
            if snapshot.active_lease_id.is_some() {
                return Err(conflict(
                    "commit_decision acquire requires no active lease on the authoritative snapshot",
                ));
            }
            if has_open_lease_for_work(state, &req.context.record.work_id) {
                return Err(conflict(
                    "commit_decision acquire requires no open lease on the authoritative snapshot",
                ));
            }
        }
        LeaseEffect::Keep | LeaseEffect::Renew | LeaseEffect::Release | LeaseEffect::None => {
            let Some(lease_id) = req.context.record.lease_id.as_ref() else {
                return Ok(());
            };
            let live_lease = state
                .leases
                .get(lease_id.as_str())
                .filter(|lease| lease.released_at.is_none());
            if live_lease.is_none() {
                return Err(conflict(
                    "commit_decision requires a live authoritative lease",
                ));
            }
            if snapshot.active_lease_id.as_ref() != Some(lease_id) {
                return Err(conflict(
                    "commit_decision lease does not match authoritative snapshot",
                ));
            }
        }
    }

    Ok(())
}

fn acquire_claim_lease(
    state: &mut MemoryState,
    work_id: &WorkId,
    agent_id: &AgentId,
    lease_id: &LeaseId,
    acquired_at: SystemTime,
    capability: &str,
) -> Result<WorkLease, StoreError> {
    let snapshot = state
        .snapshots
        .get(work_id.as_str())
        .cloned()
        .ok_or_else(|| not_found(capability, work_id))?;
    let agent = state
        .agents
        .get(agent_id.as_str())
        .cloned()
        .ok_or_else(|| conflict(&format!("{capability} requires a registered agent")))?;

    if agent.company_id != snapshot.company_id {
        return Err(conflict(&format!(
            "{capability} rejects agent/work company boundary violations",
        )));
    }

    match agent.status {
        AgentStatus::Active => {}
        AgentStatus::Paused => {
            return Err(conflict(&format!("{capability} rejects paused agents")));
        }
        AgentStatus::Terminated => {
            return Err(conflict(&format!("{capability} rejects terminated agents")));
        }
    }

    if snapshot.status != WorkStatus::Todo {
        return Err(conflict(&format!(
            "{capability} requires a todo snapshot without an open lease",
        )));
    }

    if snapshot.active_lease_id.is_some() || has_open_lease_for_work(state, work_id) {
        return Err(conflict("work already has an open lease"));
    }

    let run_id = state.ensure_runnable_run(&snapshot, agent_id, acquired_at);
    let lease = WorkLease {
        lease_id: lease_id.clone(),
        company_id: snapshot.company_id.clone(),
        work_id: snapshot.work_id.clone(),
        agent_id: agent_id.clone(),
        run_id: Some(run_id),
        acquired_at,
        expires_at: None,
        released_at: None,
        release_reason: None,
    };

    state
        .leases
        .insert(lease_id.as_str().to_owned(), lease.clone());
    if let Some(snapshot) = state.snapshots.get_mut(work_id.as_str()) {
        snapshot.status = WorkStatus::Doing;
        snapshot.assignee_agent_id = Some(agent_id.clone());
        snapshot.active_lease_id = Some(lease_id.clone());
        snapshot.rev += 1;
        snapshot.updated_at = acquired_at;
    }
    Ok(lease)
}

fn has_open_lease_for_work(state: &MemoryState, work_id: &WorkId) -> bool {
    state
        .leases
        .values()
        .any(|lease| lease.work_id == *work_id && lease.released_at.is_none())
}

fn claim_actor_agent_id(
    record: &TransitionRecord,
    capability: &str,
) -> Result<AgentId, StoreError> {
    if record.actor_kind != ActorKind::Agent {
        return Err(conflict(&format!(
            "{capability} acquire requires an agent actor",
        )));
    }

    Ok(AgentId::from(record.actor_id.as_str()))
}

#[cfg(test)]
mod tests {
    use crate::model::{
        workspace_fingerprint, ActorId, ActorKind, BillingKind, ConsumptionUsage, ContractSetId,
        DecisionOutcome, EvidenceBundle, EvidenceInline, EvidenceRef, GateResult, GateSpec,
        LeaseEffect, PendingWakeEffect, ProofHint, ProofHintKind, RecordId, RuntimeKind, SessionId,
        TaskSession, TransitionDecision, TransitionKind, TransitionRecord, WorkKind, WorkPatch,
    };

    use super::*;

    fn merge_wake_req(work_id: &str, reason: &str, obligations: &[&str]) -> MergeWakeReq {
        MergeWakeReq {
            work_id: WorkId::from(work_id),
            actor_kind: ActorKind::Board,
            actor_id: ActorId::from("board"),
            source: "manual".to_owned(),
            reason: reason.to_owned(),
            obligations: obligations.iter().map(|item| (*item).to_owned()).collect(),
        }
    }

    fn usage() -> ConsumptionUsage {
        ConsumptionUsage {
            input_tokens: 120,
            output_tokens: 48,
            run_seconds: 3,
            estimated_cost_cents: Some(7),
        }
    }

    #[test]
    fn claim_lease_acquires_single_open_lease_and_conflicts_on_second_claim() {
        let store = MemoryStore::demo();

        let first = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                lease_id: LeaseId::from("lease-claim-1"),
            })
            .expect("first claim should succeed");
        let work = store
            .read_work(Some(&WorkId::from(DEMO_TODO_WORK_ID)))
            .expect("work should remain readable");

        assert_eq!(first.lease.lease_id, LeaseId::from("lease-claim-1"));
        assert_eq!(work.items[0].status, WorkStatus::Doing);
        assert_eq!(
            work.items[0].active_lease_id.as_deref(),
            Some("lease-claim-1")
        );

        let error = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from("00000000-0000-4000-8000-000000000099"),
                lease_id: LeaseId::from("lease-claim-2"),
            })
            .expect_err("second open claim should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
    }

    #[test]
    fn claim_lease_rejects_paused_agent() {
        let store = MemoryStore::demo();

        let error = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from(DEMO_PAUSED_AGENT_ID),
                lease_id: LeaseId::from("lease-paused"),
            })
            .expect_err("paused agent claim should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("paused"));
    }

    #[test]
    fn create_agent_registers_profile_and_active_status() {
        let store = MemoryStore::demo();

        let created = store
            .create_agent(CreateAgentReq {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                name: "Release Operator".to_owned(),
                role: "release_manager".to_owned(),
            })
            .expect("create agent should succeed");

        let agents = store.read_agents();
        let registered = agents
            .registered_agents
            .iter()
            .find(|agent| agent.agent_id == created.agent_id.as_str())
            .expect("created agent should be readable");

        assert_eq!(created.status, AgentStatus::Active);
        assert_eq!(registered.name, "Release Operator");
        assert_eq!(registered.role, "release_manager");
        assert_eq!(registered.status, AgentStatus::Active);
    }

    #[test]
    fn create_work_persists_backlog_snapshot_with_tree_metadata() {
        let store = MemoryStore::demo();

        let created = store
            .create_work(CreateWorkReq {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: Some(WorkId::from(DEMO_TODO_WORK_ID)),
                kind: WorkKind::Decision,
                title: "Decide release".to_owned(),
                body: "Compare rollout options".to_owned(),
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            })
            .expect("create work should succeed");

        let work = store
            .read_work(Some(&created.snapshot.work_id))
            .expect("created work should be readable");

        assert_eq!(created.snapshot.status, WorkStatus::Backlog);
        assert_eq!(work.items[0].parent_id.as_deref(), Some(DEMO_TODO_WORK_ID));
        assert_eq!(work.items[0].kind, WorkKind::Decision);
        assert_eq!(work.items[0].title, "Decide release");
        assert_eq!(work.items[0].body, "Compare rollout options");
        assert_eq!(work.items[0].rev, 0);
        assert_eq!(work.items[0].contract_set_id, DEMO_CONTRACT_SET_ID);
        assert_eq!(work.items[0].contract_rev, 1);
        assert_eq!(
            work.items[0].contract_name.as_deref(),
            Some("axiomnexus-rust-default")
        );
        assert_eq!(
            work.items[0].contract_status,
            Some(ContractSetStatus::Active)
        );
    }

    #[test]
    fn new_company_can_activate_contract_and_create_work() {
        let store = MemoryStore::demo();
        let company = store
            .create_company(CreateCompanyReq {
                name: "Scenario Labs".to_owned(),
                description: "http onboarding".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company create should succeed");
        let rules = store.read_contracts().rules;

        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "scenario-contract".to_owned(),
                rules,
            })
            .expect("contract draft should succeed");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("contract activation should succeed");

        let companies = store.read_companies();
        let created_company = companies
            .items
            .iter()
            .find(|item| item.company_id == company.profile.company_id.as_str())
            .expect("created company should be readable");
        let contract_set_id = created_company
            .active_contract_set_id
            .as_deref()
            .expect("new company should expose active contract");
        let created_work = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Scenario Task".to_owned(),
                body: "Exercise queue/wake".to_owned(),
                contract_set_id: ContractSetId::from(contract_set_id),
            })
            .expect("create work should succeed for the new company");
        let work = store
            .read_work(Some(&created_work.snapshot.work_id))
            .expect("created work should be readable");

        assert_eq!(draft.revision, 1);
        assert_ne!(contract_set_id, DEMO_CONTRACT_SET_ID);
        assert_eq!(created_company.active_contract_revision, Some(1));
        assert_eq!(work.items[0].rev, 0);
        assert_eq!(work.items[0].contract_rev, 1);
        assert_eq!(
            work.items[0].contract_name.as_deref(),
            Some("scenario-contract")
        );
        assert_eq!(
            work.items[0].contract_status,
            Some(ContractSetStatus::Active)
        );
    }

    #[test]
    fn work_context_and_runtime_turn_use_work_pinned_contract_after_foreign_activation() {
        let store = MemoryStore::demo();
        let company = store
            .create_company(CreateCompanyReq {
                name: "Foreign Runtime".to_owned(),
                description: "secondary company".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company create should succeed");
        let rules = store.read_contracts().rules;
        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "foreign-contract".to_owned(),
                rules,
            })
            .expect("foreign contract draft should succeed");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id,
                revision: draft.revision,
            })
            .expect("foreign contract activation should succeed");

        let context = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("demo work context should load");
        let runtime_turn = store
            .load_runtime_turn(&RunId::from("run-1"))
            .expect("demo run should still load");

        assert_eq!(context.contract.company_id.as_str(), DEMO_COMPANY_ID);
        assert_eq!(
            context.contract.contract_set_id.as_str(),
            DEMO_CONTRACT_SET_ID
        );
        assert_eq!(context.contract.name, "axiomnexus-rust-default");
        assert_eq!(runtime_turn.contract.company_id.as_str(), DEMO_COMPANY_ID);
        assert_eq!(
            runtime_turn.contract.contract_set_id.as_str(),
            DEMO_CONTRACT_SET_ID
        );
    }

    #[test]
    fn update_work_rejects_tree_cycles() {
        let store = MemoryStore::demo();
        let parent = store
            .create_work(CreateWorkReq {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: Some(WorkId::from(DEMO_TODO_WORK_ID)),
                kind: WorkKind::Project,
                title: "Parent project".to_owned(),
                body: String::new(),
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            })
            .expect("parent create should succeed");
        let child = store
            .create_work(CreateWorkReq {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: Some(parent.snapshot.work_id.clone()),
                kind: WorkKind::Task,
                title: "Child task".to_owned(),
                body: String::new(),
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            })
            .expect("child create should succeed");

        let error = store
            .update_work(UpdateWorkReq {
                work_id: parent.snapshot.work_id,
                parent_id: Some(child.snapshot.work_id),
                title: "Parent project".to_owned(),
                body: String::new(),
            })
            .expect_err("cycle update should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("cycle"));
    }

    #[test]
    fn set_agent_status_rejects_resuming_terminated_agent() {
        let store = MemoryStore::demo();

        let error = store
            .set_agent_status(SetAgentStatusReq {
                agent_id: AgentId::from(DEMO_TERMINATED_AGENT_ID),
                status: AgentStatus::Active,
            })
            .expect_err("terminated agent resume should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("terminated"));
    }

    #[test]
    fn claim_lease_rejects_terminated_agent() {
        let store = MemoryStore::demo();

        let error = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from(DEMO_TERMINATED_AGENT_ID),
                lease_id: LeaseId::from("lease-terminated"),
            })
            .expect_err("terminated agent claim should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("terminated"));
    }

    #[test]
    fn claim_lease_rejects_company_boundary_violation() {
        let store = MemoryStore::demo();

        let error = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from(DEMO_FOREIGN_AGENT_ID),
                lease_id: LeaseId::from("lease-foreign"),
            })
            .expect_err("foreign-company agent claim should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("company boundary"));
    }

    #[test]
    fn claim_lease_creates_running_run_for_new_claim() {
        let store = MemoryStore::demo();

        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                lease_id: LeaseId::from("lease-run-1"),
            })
            .expect("claim should create runnable run");

        let state = store.state.borrow();
        let run_id = claimed
            .lease
            .run_id
            .as_ref()
            .expect("claim lease should carry run id");
        let run = state
            .runs
            .get(run_id.as_str())
            .expect("created run should be stored");
        let claim_record = store
            .load_transition_records(&WorkId::from(DEMO_TODO_WORK_ID))
            .expect("claim records should load")
            .into_iter()
            .find(|record| record.kind == TransitionKind::Claim)
            .expect("claim transition should persist");

        assert_eq!(run.work_id, WorkId::from(DEMO_TODO_WORK_ID));
        assert_eq!(run.agent_id, AgentId::from(DEMO_AGENT_ID));
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(claim_record.expected_rev, 0);
        assert_eq!(claim_record.after_status, Some(WorkStatus::Doing));
    }

    #[test]
    fn claim_lease_links_existing_runnable_run_for_work() {
        let mut state = MemoryState::demo();
        state.runs.insert(
            "run-queued".to_owned(),
            RegisteredRun {
                run_id: RunId::from("run-queued"),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                status: RunStatus::Queued,
                created_at: SystemTime::UNIX_EPOCH,
                updated_at: SystemTime::UNIX_EPOCH,
            },
        );
        let store = MemoryStore::with_state(state);

        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                lease_id: LeaseId::from("lease-run-queued"),
            })
            .expect("claim should reuse queued run");

        let state = store.state.borrow();
        let run = state
            .runs
            .get("run-queued")
            .expect("queued run should remain stored");

        assert_eq!(claimed.lease.run_id, Some(RunId::from("run-queued")));
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(run.agent_id, AgentId::from(DEMO_AGENT_ID));
    }

    #[test]
    fn merge_wake_persists_coalesced_pending_wake() {
        let store = MemoryStore::demo();

        let merged = store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "gate failed",
                &["run tests", "run fmt"],
            ))
            .expect("wake merge should succeed");

        assert_eq!(merged.count, 1);

        let merged_again = store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "another gate failed",
                &["run fmt"],
            ))
            .expect("second wake merge should succeed");

        assert_eq!(merged_again.count, 2);
        assert_eq!(merged_again.obligation_json.len(), 2);
        let state = store.state.borrow();
        assert_eq!(state.comments.len(), 2);
        assert_eq!(state.comments[0].author_kind, ActorKind::Board);
        assert_eq!(state.comments[0].author_id, ActorId::from("board"));
        assert!(state.comments[1].body.contains("manual wake merged"));
    }

    #[test]
    fn merge_wake_creates_single_queued_run_when_work_has_no_open_lease() {
        let store = MemoryStore::demo();

        store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "new requirement",
                &["cargo test"],
            ))
            .expect("wake merge should succeed");

        let state = store.state.borrow();
        let runnable_runs = state
            .runs
            .values()
            .filter(|run| {
                run.work_id == WorkId::from(DEMO_TODO_WORK_ID) && run.status.is_runnable()
            })
            .collect::<Vec<_>>();

        assert_eq!(runnable_runs.len(), 1);
        assert_eq!(runnable_runs[0].status, RunStatus::Queued);
        assert_eq!(runnable_runs[0].agent_id, AgentId::from(DEMO_AGENT_ID));
    }

    #[test]
    fn merge_wake_for_paused_agent_keeps_pending_wake_without_creating_run() {
        let store = MemoryStore::demo();
        store
            .set_agent_status(SetAgentStatusReq {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                status: AgentStatus::Paused,
            })
            .expect("pause should succeed");

        let merged = store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "paused agent wake",
                &["cargo test"],
            ))
            .expect("wake merge should succeed");

        let state = store.state.borrow();
        let runnable_runs = state
            .runs
            .values()
            .filter(|run| {
                run.work_id == WorkId::from(DEMO_TODO_WORK_ID) && run.status.is_runnable()
            })
            .collect::<Vec<_>>();

        assert_eq!(merged.count, 1);
        assert!(state.pending_wakes.contains_key(DEMO_TODO_WORK_ID));
        assert!(runnable_runs.is_empty());
    }

    #[test]
    fn merge_wake_reuses_existing_runnable_run_without_queue_fanout() {
        let store = MemoryStore::demo();

        store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "first wake",
                &["cargo test"],
            ))
            .expect("first wake should succeed");
        store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "second wake",
                &["cargo fmt"],
            ))
            .expect("second wake should succeed");

        let state = store.state.borrow();
        let runnable_runs = state
            .runs
            .values()
            .filter(|run| {
                run.work_id == WorkId::from(DEMO_TODO_WORK_ID) && run.status.is_runnable()
            })
            .collect::<Vec<_>>();

        assert_eq!(runnable_runs.len(), 1);
        assert_eq!(state.pending_wakes[DEMO_TODO_WORK_ID].count, 2);
    }

    #[test]
    fn reap_timed_out_running_run_releases_expired_lease_and_queues_follow_up() {
        let store = MemoryStore::demo();
        store
            .merge_wake(merge_wake_req(
                DEMO_DOING_WORK_ID,
                "retry timed out run",
                &["retry"],
            ))
            .expect("wake merge should persist pending work");

        let reaped = store
            .reap_timed_out_runs(Duration::from_secs(5))
            .expect("reaper should succeed");

        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].run_id, RunId::from("run-1"));
        assert_eq!(reaped[0].work_id, WorkId::from(DEMO_DOING_WORK_ID));
        assert_eq!(
            reaped[0].released_lease_id,
            Some(LeaseId::from(DEMO_LEASE_ID))
        );
        assert_eq!(reaped[0].follow_up_run_id, Some(RunId::from("run-2")));

        let state = store.state.borrow();
        let failed_run = state
            .runs
            .get("run-1")
            .expect("stale run should remain stored");
        let follow_up = state
            .runs
            .get("run-2")
            .expect("reaper should queue a single follow-up run");
        let lease = state
            .leases
            .get(DEMO_LEASE_ID)
            .expect("lease should remain stored");
        let timeout_record = state
            .transition_records
            .iter()
            .find(|record| record.kind == TransitionKind::TimeoutRequeue)
            .expect("timeout record should persist");
        let snapshot = state
            .snapshots
            .get(DEMO_DOING_WORK_ID)
            .expect("work snapshot should remain stored");

        assert_eq!(failed_run.status, RunStatus::TimedOut);
        assert_eq!(follow_up.status, RunStatus::Queued);
        assert_eq!(follow_up.work_id, WorkId::from(DEMO_DOING_WORK_ID));
        assert_eq!(lease.release_reason, Some(LeaseReleaseReason::Expired));
        assert!(lease.released_at.is_some());
        assert!(lease.expires_at.is_some());
        assert_eq!(timeout_record.actor_kind, ActorKind::System);
        assert_eq!(timeout_record.actor_id, ActorId::from("system"));
        assert_eq!(timeout_record.before_status, WorkStatus::Doing);
        assert_eq!(timeout_record.after_status, Some(WorkStatus::Todo));
        assert_eq!(snapshot.status, WorkStatus::Todo);
        assert_eq!(snapshot.active_lease_id, None);
        assert_eq!(snapshot.assignee_agent_id, None);
        assert_eq!(snapshot.rev, 2);
        assert_eq!(state.pending_wakes[DEMO_DOING_WORK_ID].count, 1);
        assert_eq!(state.comments.len(), 2);
        assert_eq!(state.comments[0].author_kind, ActorKind::Board);
        assert_eq!(state.comments[0].author_id, ActorId::from("board"));
        assert!(state.comments[0].body.contains("manual wake merged"));
        assert_eq!(state.comments[1].author_kind, ActorKind::System);
        assert_eq!(state.comments[1].author_id, ActorId::from("system"));
        assert!(state.comments[1].body.contains("timed out run run-1"));
        assert!(state.comments[1].body.contains("follow-up run-2 queued"));
    }

    #[test]
    fn reap_timed_out_running_run_without_pending_wake_does_not_fan_out_new_run() {
        let store = MemoryStore::demo();

        let reaped = store
            .reap_timed_out_runs(Duration::from_secs(5))
            .expect("reaper should succeed");

        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].follow_up_run_id, None);

        let state = store.state.borrow();
        let failed_run = state
            .runs
            .get("run-1")
            .expect("stale run should remain stored");
        let runnable_runs = state
            .runs
            .values()
            .filter(|run| {
                run.work_id == WorkId::from(DEMO_DOING_WORK_ID) && run.status.is_runnable()
            })
            .collect::<Vec<_>>();

        assert_eq!(failed_run.status, RunStatus::TimedOut);
        assert!(runnable_runs.is_empty());
    }

    #[test]
    fn read_models_reflect_live_store_state() {
        let store = MemoryStore::demo();
        store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "gate failed",
                &["cargo test"],
            ))
            .expect("wake merge should succeed");
        store
            .record_consumption(RecordConsumptionReq {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                run_id: RunId::from("run-1"),
                billing_kind: BillingKind::Api,
                usage: usage(),
            })
            .expect("consumption event should persist");
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-queued"),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-queued".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 2,
                last_record_id: None,
                last_decision_summary: Some("queued session".to_owned()),
                last_gate_summary: None,
                updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(20),
            })
            .expect("session should persist");

        let board = store.read_board();
        let work = store
            .read_work(Some(&WorkId::from(DEMO_TODO_WORK_ID)))
            .expect("work read should succeed");
        let agents = store.read_agents();
        let activity = store.read_activity();
        let contracts = store.read_contracts();

        assert_eq!(board.running_agents, vec![DEMO_AGENT_ID.to_owned()]);
        assert_eq!(board.running_runs.len(), 1);
        assert_eq!(board.running_runs[0].run_id, "run-1");
        assert_eq!(board.running_runs[0].work_id, DEMO_DOING_WORK_ID);
        assert_eq!(
            board.running_runs[0].lease_id.as_deref(),
            Some(DEMO_LEASE_ID)
        );
        assert_eq!(board.pending_wakes, vec![DEMO_TODO_WORK_ID.to_owned()]);
        assert_eq!(board.pending_wake_details.len(), 1);
        assert_eq!(board.pending_wake_details[0].work_id, DEMO_TODO_WORK_ID);
        assert_eq!(board.pending_wake_details[0].count, 1);
        assert_eq!(board.pending_wake_details[0].latest_reason, "gate failed");
        assert_eq!(
            board.pending_wake_details[0].obligations,
            vec!["cargo test".to_owned()]
        );
        assert!(board.recent_transition_details.is_empty());
        assert!(board.recent_gate_failure_details.is_empty());
        assert_eq!(work.items.len(), 1);
        assert_eq!(
            work.items[0].pending_obligations,
            vec!["cargo test".to_owned()]
        );
        assert_eq!(agents.active_agents, vec![DEMO_AGENT_ID.to_owned()]);
        assert!(agents
            .registered_agents
            .iter()
            .any(|agent| agent.agent_id == DEMO_AGENT_ID && agent.status == AgentStatus::Active));
        assert_eq!(agents.recent_runs[0].run_id, "run-2");
        assert_eq!(agents.recent_runs[0].status, "queued");
        assert_eq!(agents.current_sessions[0].agent_id, DEMO_AGENT_ID);
        assert_eq!(
            agents.current_sessions[0].runtime_session_id,
            "runtime-queued"
        );
        assert_eq!(board.consumption_summary.total_turns, 1);
        assert_eq!(board.consumption_summary.total_run_seconds, 3);
        assert_eq!(
            agents
                .consumption_by_agent
                .iter()
                .find(|summary| summary.agent_id == DEMO_AGENT_ID)
                .expect("agent rollup should exist")
                .total_input_tokens,
            120
        );
        assert!(activity.entries.len() >= 2);
        assert!(activity
            .entries
            .iter()
            .any(|entry| entry.event_kind == "comment"));
        assert!(activity
            .entries
            .iter()
            .any(|entry| entry.event_kind == "run"));
        assert_eq!(contracts.contract_set_id, DEMO_CONTRACT_SET_ID);
        assert_eq!(contracts.name, "axiomnexus-rust-default");
        assert_eq!(contracts.status, ContractSetStatus::Active);
        assert_eq!(contracts.revisions.len(), 3);
        assert_eq!(contracts.revisions[0].status, ContractSetStatus::Retired);
        assert!(!contracts.rules.is_empty());
    }

    #[test]
    fn read_run_returns_status_and_current_session() {
        let store = MemoryStore::demo();
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-running"),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-running".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 2,
                last_record_id: None,
                last_decision_summary: Some("running session".to_owned()),
                last_gate_summary: None,
                updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(30),
            })
            .expect("session should persist");

        let run = store
            .read_run(&RunId::from("run-1"))
            .expect("run read should succeed");

        assert_eq!(run.run_id, "run-1");
        assert_eq!(run.agent_id, DEMO_AGENT_ID);
        assert_eq!(run.work_id, DEMO_DOING_WORK_ID);
        assert_eq!(run.status, "running");
        assert_eq!(
            run.current_session
                .as_ref()
                .expect("session should be attached")
                .runtime_session_id,
            "runtime-running"
        );
    }

    #[test]
    fn read_board_only_counts_running_runs_as_running_agents() {
        let mut state = MemoryState::demo();
        state.runs.clear();
        state.leases.clear();
        if let Some(snapshot) = state.snapshots.get_mut(DEMO_DOING_WORK_ID) {
            snapshot.status = WorkStatus::Todo;
            snapshot.assignee_agent_id = None;
            snapshot.active_lease_id = None;
        }
        let store = MemoryStore::with_state(state);
        store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "gate failed",
                &["cargo test"],
            ))
            .expect("wake merge should create only a queued run");

        let board = store.read_board();

        assert!(board.running_agents.is_empty());
        assert!(board.running_runs.is_empty());
        assert_eq!(board.pending_wakes, vec![DEMO_TODO_WORK_ID.to_owned()]);
        assert_eq!(board.pending_wake_details.len(), 1);
        assert_eq!(board.pending_wake_details[0].work_id, DEMO_TODO_WORK_ID);
    }

    #[test]
    fn read_board_projects_recent_gate_failure_details() {
        let store = MemoryStore::demo();
        let expected_rev = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load")
            .snapshot
            .rev;
        let rejected = TransitionDecision {
            outcome: DecisionOutcome::Rejected,
            reasons: vec![crate::model::ReasonCode::GateFailed],
            next_snapshot: None,
            lease_effect: LeaseEffect::Keep,
            pending_wake_effect: PendingWakeEffect::Retain,
            gate_results: vec![GateResult {
                gate: GateSpec::AllRequiredObligationsResolved,
                passed: false,
                detail: "all pending obligations must be resolved".to_owned(),
            }],
            evidence: EvidenceBundle::default(),
            summary: "gate denied completion".to_owned(),
        };
        let record = crate::model::TransitionRecord {
            record_id: RecordId::from("record-rejected"),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::from(DEMO_AGENT_ID),
            run_id: None,
            session_id: None,
            lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
            expected_rev,
            contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            contract_rev: 1,
            before_status: WorkStatus::Doing,
            after_status: None,
            outcome: rejected.outcome,
            reasons: rejected.reasons.clone(),
            kind: TransitionKind::Complete,
            patch: crate::model::WorkPatch {
                summary: "attempt complete".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            gate_results: rejected.gate_results.clone(),
            evidence: rejected.evidence.clone(),
            evidence_inline: Some(crate::model::EvidenceInline {
                summary: "gate denied completion".to_owned(),
            }),
            evidence_refs: Vec::new(),
            happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(40),
        };
        store
            .commit_decision(CommitDecisionReq::new(rejected, record, None))
            .expect("rejected decision should persist record");

        let board = store.read_board();

        assert_eq!(board.recent_gate_failures[0], "record-rejected");
        assert_eq!(board.recent_gate_failure_details.len(), 1);
        assert_eq!(
            board.recent_gate_failure_details[0].record_id,
            "record-rejected"
        );
        assert_eq!(
            board.recent_gate_failure_details[0].work_id,
            DEMO_DOING_WORK_ID
        );
        assert_eq!(board.recent_gate_failure_details[0].outcome, "rejected");
        assert!(!board.recent_gate_failure_details[0].failed_gates.is_empty());
        assert!(board.recent_gate_failure_details[0]
            .failed_gates
            .iter()
            .any(|detail| detail == "all pending obligations must be resolved"));
    }

    #[test]
    fn read_models_project_transition_record_details_into_board_and_work_audit() {
        let store = MemoryStore::demo();
        let expected_rev = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load")
            .snapshot
            .rev;
        let rejected = TransitionDecision {
            outcome: DecisionOutcome::Rejected,
            reasons: vec![crate::model::ReasonCode::GateFailed],
            next_snapshot: None,
            lease_effect: LeaseEffect::Keep,
            pending_wake_effect: PendingWakeEffect::Retain,
            gate_results: vec![GateResult {
                gate: GateSpec::AllRequiredObligationsResolved,
                passed: false,
                detail: "all pending obligations must be resolved".to_owned(),
            }],
            evidence: EvidenceBundle::default(),
            summary: "gate denied completion".to_owned(),
        };
        let record = crate::model::TransitionRecord {
            record_id: RecordId::from("record-transition-projection"),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::from(DEMO_AGENT_ID),
            run_id: None,
            session_id: None,
            lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
            expected_rev,
            contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            contract_rev: 1,
            before_status: WorkStatus::Doing,
            after_status: None,
            outcome: rejected.outcome,
            reasons: rejected.reasons.clone(),
            kind: TransitionKind::Complete,
            patch: crate::model::WorkPatch {
                summary: "attempt complete".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            gate_results: rejected.gate_results.clone(),
            evidence: rejected.evidence.clone(),
            evidence_inline: Some(crate::model::EvidenceInline {
                summary: "gate denied completion".to_owned(),
            }),
            evidence_refs: Vec::new(),
            happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(41),
        };
        store
            .commit_decision(CommitDecisionReq::new(rejected, record, None))
            .expect("rejected decision should persist record");

        let board = store.read_board();
        let work = store
            .read_work(Some(&WorkId::from(DEMO_DOING_WORK_ID)))
            .expect("work read should succeed");

        assert_eq!(
            board.recent_transition_records[0],
            "record-transition-projection"
        );
        assert_eq!(board.recent_transition_details.len(), 1);
        assert_eq!(
            board.recent_transition_details[0].record_id,
            "record-transition-projection"
        );
        assert_eq!(board.recent_transition_details[0].kind, "complete");
        assert_eq!(board.recent_transition_details[0].outcome, "rejected");
        assert_eq!(
            board.recent_transition_details[0].summary,
            "gate denied completion"
        );
        assert!(work.items[0].audit_entries.iter().any(|entry| {
            entry.event_kind == "transition"
                && entry.summary == "gate denied completion"
                && entry.outcome.as_deref() == Some("rejected")
                && entry.before_status == Some(WorkStatus::Doing)
                && entry.after_status.is_none()
        }));
    }

    #[test]
    fn read_work_returns_recent_20_work_scoped_audit_entries() {
        let store = MemoryStore::demo();
        store
            .merge_wake(merge_wake_req(
                DEMO_TODO_WORK_ID,
                "gate failed",
                &["cargo test"],
            ))
            .expect("wake merge should seed target work audit");

        for seq in 0..25 {
            store
                .append_comment(AppendCommentReq {
                    company_id: CompanyId::from(DEMO_COMPANY_ID),
                    work_id: WorkId::from(DEMO_TODO_WORK_ID),
                    author_kind: ActorKind::Board,
                    author_id: ActorId::from("board"),
                    body: format!("todo-{seq}"),
                })
                .expect("target comments should append");
        }

        for seq in 0..25 {
            store
                .append_comment(AppendCommentReq {
                    company_id: CompanyId::from(DEMO_COMPANY_ID),
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    author_kind: ActorKind::Board,
                    author_id: ActorId::from("board"),
                    body: format!("noise-{seq}"),
                })
                .expect("noise comments should append");
        }

        let work = store
            .read_work(Some(&WorkId::from(DEMO_TODO_WORK_ID)))
            .expect("work read should succeed");

        assert_eq!(work.items[0].audit_entries.len(), 20);
        assert!(work.items[0]
            .audit_entries
            .iter()
            .all(|entry| entry.work_id == DEMO_TODO_WORK_ID));
        assert!(work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.event_kind == "comment" && entry.summary == "todo-24"));
        assert!(!work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.summary == "todo-0"));
        assert!(!work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.summary.starts_with("noise-")));
    }

    #[test]
    fn commit_decision_updates_snapshot_record_session_and_pending_wake_atomically() {
        let store = MemoryStore::with_state(MemoryState::demo());
        store
            .merge_wake(merge_wake_req(
                DEMO_DOING_WORK_ID,
                "needs verification",
                &["cargo test"],
            ))
            .expect("seed wake should succeed");

        let context = store
            .load_context(&WorkId::from(DEMO_DOING_WORK_ID))
            .expect("context should load");
        let intent = crate::model::TransitionIntent {
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            lease_id: LeaseId::from(DEMO_LEASE_ID),
            expected_rev: context.snapshot.rev,
            kind: TransitionKind::Block,
            patch: WorkPatch {
                summary: "blocked".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: vec!["needs reviewer".to_owned()],
            },
            note: Some("blocked by review".to_owned()),
            proof_hints: vec![ProofHint {
                kind: ProofHintKind::Summary,
                value: "blocked".to_owned(),
            }],
        };
        let decision = kernel::decide_transition(
            &context.snapshot,
            context.lease.as_ref(),
            context.pending_wake.as_ref(),
            &context.contract,
            &EvidenceBundle::default(),
            &intent,
        );
        let session = TaskSession {
            session_id: SessionId::from("session-1"),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            agent_id: AgentId::from(DEMO_AGENT_ID),
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            runtime: RuntimeKind::Coclai,
            runtime_session_id: "runtime-1".to_owned(),
            cwd: "/repo".to_owned(),
            workspace_fingerprint: workspace_fingerprint("/repo"),
            contract_rev: context.contract.revision,
            last_record_id: Some(RecordId::from("record-1")),
            last_decision_summary: Some(decision.summary.clone()),
            last_gate_summary: None,
            updated_at: SystemTime::UNIX_EPOCH,
        };
        let record = TransitionRecord {
            record_id: RecordId::from("record-1"),
            company_id: CompanyId::from(DEMO_COMPANY_ID),
            work_id: WorkId::from(DEMO_DOING_WORK_ID),
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::from(DEMO_AGENT_ID),
            run_id: None,
            session_id: Some(session.session_id.clone()),
            lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
            expected_rev: context.snapshot.rev,
            contract_set_id: context.snapshot.contract_set_id.clone(),
            contract_rev: context.snapshot.contract_rev,
            before_status: context.snapshot.status,
            after_status: decision
                .next_snapshot
                .as_ref()
                .map(|snapshot| snapshot.status),
            outcome: DecisionOutcome::Accepted,
            reasons: decision.reasons.clone(),
            kind: TransitionKind::Block,
            patch: intent.patch.clone(),
            gate_results: vec![GateResult {
                gate: crate::model::GateSpec::ManualNotePresent,
                passed: true,
                detail: "note present".to_owned(),
            }],
            evidence: decision.evidence.clone(),
            evidence_inline: Some(EvidenceInline {
                summary: decision.summary.clone(),
            }),
            evidence_refs: Vec::<EvidenceRef>::new(),
            happened_at: SystemTime::UNIX_EPOCH,
        };

        let result = store
            .commit_decision(CommitDecisionReq::new(
                TransitionDecision {
                    pending_wake_effect: PendingWakeEffect::Clear,
                    lease_effect: LeaseEffect::Release,
                    ..decision.clone()
                },
                record,
                Some(session.clone()),
            ))
            .expect("commit_decision should succeed");

        assert_eq!(
            result
                .snapshot
                .as_ref()
                .expect("snapshot should persist")
                .status,
            WorkStatus::Blocked
        );
        assert!(result.pending_wake.is_none());
        assert!(result.lease.is_none());
        assert_ne!(
            result
                .session
                .as_ref()
                .expect("session should persist")
                .updated_at,
            session.updated_at
        );
        assert_eq!(
            result
                .activity_event
                .expect("activity event should persist")
                .summary,
            "Block Accepted with next status Blocked"
        );

        let state = store.state.borrow();
        let persisted_record = state
            .transition_records
            .iter()
            .find(|item| item.record_id == RecordId::from("record-1"))
            .expect("record should persist");
        let persisted_snapshot = state
            .snapshots
            .get(DEMO_DOING_WORK_ID)
            .expect("snapshot should persist");
        let persisted_session = state
            .sessions
            .get(&session_key_parts(
                &AgentId::from(DEMO_AGENT_ID),
                &WorkId::from(DEMO_DOING_WORK_ID),
            ))
            .expect("session should persist");

        assert_ne!(persisted_record.happened_at, SystemTime::UNIX_EPOCH);
        assert_eq!(
            persisted_record
                .session_id
                .as_ref()
                .map(|session| session.as_str()),
            Some("session-1")
        );
        assert_eq!(persisted_record.run_id, None);
        assert_eq!(persisted_snapshot.updated_at, persisted_record.happened_at);
        assert_eq!(persisted_session.updated_at, persisted_record.happened_at);
    }

    #[test]
    fn memory_commit_stage_helpers_match_surreal_stage_names() {
        let memory_src = include_str!("store/commit.rs");
        let surreal_src = include_str!("../surreal/store/commit.rs");

        for helper in [
            "load_commit_authoritative_state(",
            "prepare_commit_decision(",
            "load_commit_decision_result(",
        ] {
            assert!(memory_src.contains(helper));
            assert!(surreal_src.contains(helper));
        }
        assert!(memory_src.contains("execute_commit_decision_apply("));
        assert!(surreal_src.contains("execute_commit_decision_transaction("));
    }

    #[test]
    fn memory_commit_apply_appends_record_before_projection_updates() {
        let src = include_str!("store/commit.rs");
        let record_push = src
            .find("next.transition_records.push(req.context.record.clone());")
            .expect("record append should exist");
        let activity_push = src
            .find("next.activity_events.push(activity_event.clone());")
            .expect("activity append should exist");
        let snapshot_apply = src
            .find("if let Some(snapshot) = req.decision.next_snapshot.clone()")
            .expect("snapshot apply should exist");
        let session_apply = src
            .find("if let Some(session) = req.effects.session.clone()")
            .expect("session apply should exist");

        assert!(record_push < snapshot_apply);
        assert!(activity_push < snapshot_apply);
        assert!(record_push < session_apply);
        assert!(activity_push < session_apply);
    }

    #[test]
    fn memory_direct_claim_and_commit_acquire_share_same_helper() {
        let store_src = include_str!("store.rs");
        let commit_src = include_str!("store/commit.rs");
        assert!(store_src.contains("let lease = acquire_claim_lease("));
        assert!(commit_src.contains("let _ = acquire_claim_lease("));
    }

    #[test]
    fn commit_decision_rejects_stale_expected_rev() {
        let store = MemoryStore::demo();
        let rejected = TransitionDecision {
            outcome: DecisionOutcome::Rejected,
            reasons: vec![crate::model::ReasonCode::RevConflict],
            next_snapshot: None,
            lease_effect: LeaseEffect::None,
            pending_wake_effect: PendingWakeEffect::Retain,
            gate_results: Vec::new(),
            evidence: EvidenceBundle::default(),
            summary: "stale rev".to_owned(),
        };

        let error = store
            .commit_decision(CommitDecisionReq::new(
                rejected,
                TransitionRecord {
                    record_id: RecordId::from("record-stale-rev"),
                    company_id: CompanyId::from(DEMO_COMPANY_ID),
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(DEMO_AGENT_ID),
                    run_id: None,
                    session_id: None,
                    lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
                    expected_rev: 999,
                    contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::RevConflict],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "stale rev".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: Vec::new(),
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "stale rev".to_owned(),
                    }),
                    evidence_refs: Vec::<EvidenceRef>::new(),
                    happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(50),
                },
                None,
            ))
            .expect_err("stale expected_rev should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("expected_rev"));
        assert!(store
            .state
            .borrow()
            .transition_records
            .iter()
            .all(|record| record.record_id != RecordId::from("record-stale-rev")));
    }

    #[test]
    fn commit_decision_rejects_stale_live_lease() {
        let store = MemoryStore::demo();
        let rejected = TransitionDecision {
            outcome: DecisionOutcome::Rejected,
            reasons: vec![crate::model::ReasonCode::StaleLease],
            next_snapshot: None,
            lease_effect: LeaseEffect::None,
            pending_wake_effect: PendingWakeEffect::Retain,
            gate_results: Vec::new(),
            evidence: EvidenceBundle::default(),
            summary: "stale lease".to_owned(),
        };

        let error = store
            .commit_decision(CommitDecisionReq::new(
                rejected,
                TransitionRecord {
                    record_id: RecordId::from("record-stale-lease"),
                    company_id: CompanyId::from(DEMO_COMPANY_ID),
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(DEMO_AGENT_ID),
                    run_id: None,
                    session_id: None,
                    lease_id: Some(LeaseId::from("lease-missing")),
                    expected_rev: 1,
                    contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::StaleLease],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "stale lease".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: Vec::new(),
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "stale lease".to_owned(),
                    }),
                    evidence_refs: Vec::<EvidenceRef>::new(),
                    happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(51),
                },
                None,
            ))
            .expect_err("stale lease should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("live authoritative lease"));
        assert!(store
            .state
            .borrow()
            .transition_records
            .iter()
            .all(|record| record.record_id != RecordId::from("record-stale-lease")));
    }

    #[test]
    fn commit_decision_persists_reason_codes_in_transition_record() {
        let store = MemoryStore::demo();
        let decision = TransitionDecision {
            outcome: DecisionOutcome::Rejected,
            reasons: vec![crate::model::ReasonCode::GateFailed],
            next_snapshot: None,
            lease_effect: LeaseEffect::Keep,
            pending_wake_effect: PendingWakeEffect::Retain,
            gate_results: vec![GateResult {
                gate: GateSpec::AllRequiredObligationsResolved,
                passed: false,
                detail: "all pending obligations must be resolved".to_owned(),
            }],
            evidence: EvidenceBundle::default(),
            summary: "gate denied completion".to_owned(),
        };

        store
            .commit_decision(CommitDecisionReq::new(
                decision.clone(),
                TransitionRecord {
                    record_id: RecordId::from("record-reasons"),
                    company_id: CompanyId::from(DEMO_COMPANY_ID),
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(DEMO_AGENT_ID),
                    run_id: None,
                    session_id: None,
                    lease_id: Some(LeaseId::from(DEMO_LEASE_ID)),
                    expected_rev: 1,
                    contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: decision.reasons.clone(),
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "attempt complete".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: decision.gate_results.clone(),
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "gate denied completion".to_owned(),
                    }),
                    evidence_refs: Vec::new(),
                    happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(52),
                },
                None,
            ))
            .expect("rejected decision should persist");

        let stored = store
            .state
            .borrow()
            .transition_records
            .iter()
            .find(|record| record.record_id == RecordId::from("record-reasons"))
            .cloned()
            .expect("transition record should persist");
        assert_eq!(stored.reasons, vec![crate::model::ReasonCode::GateFailed]);
    }

    #[test]
    fn timeout_replay_reconstructs_live_snapshot() {
        let store = MemoryStore::demo();
        store
            .merge_wake(merge_wake_req(
                DEMO_DOING_WORK_ID,
                "retry timed out run",
                &["retry"],
            ))
            .expect("wake should seed pending follow-up");
        let before = store
            .state
            .borrow()
            .snapshots
            .get(DEMO_DOING_WORK_ID)
            .cloned()
            .expect("base snapshot should exist");

        store
            .reap_timed_out_runs(Duration::from_secs(5))
            .expect("reaper should succeed");

        let state = store.state.borrow();
        let live = state
            .snapshots
            .get(DEMO_DOING_WORK_ID)
            .cloned()
            .expect("live snapshot should exist");
        let pending_wake = state
            .pending_wakes
            .get(DEMO_DOING_WORK_ID)
            .expect("pending wake should remain retained");
        let follow_up_run = state
            .runs
            .get("run-2")
            .expect("follow-up queued run should persist");
        let replay_records = state
            .transition_records
            .iter()
            .filter(|record| {
                record.work_id == WorkId::from(DEMO_DOING_WORK_ID)
                    && record.kind == TransitionKind::TimeoutRequeue
            })
            .cloned()
            .collect::<Vec<_>>();
        let replayed = crate::kernel::replay_snapshot_from_records(&before, &replay_records)
            .expect("timeout replay should succeed");

        assert_eq!(pending_wake.count, 1);
        assert_eq!(follow_up_run.status, RunStatus::Queued);
        assert_eq!(replayed, live);
    }
}
