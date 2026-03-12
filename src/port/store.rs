use std::{error::Error, fmt, time::Duration};

use serde::{Deserialize, Serialize};

use crate::model::{
    ActorId, ActorKind, AgentId, AgentStatus, BillingKind, CompanyId, CompanyProfile,
    ConsumptionUsage, ContractSet, ContractSetStatus, LeaseId, PendingWake, RunId, RuntimeKind,
    TaskSession, TransitionDecision, TransitionRecord, TransitionRule, WorkId, WorkLease,
    WorkSnapshot, WorkStatus,
};

// Aggregate adapter surface. App, boot, and transport code should depend on the
// narrower role traits below so each path only sees the store capabilities it uses.
pub(crate) trait StorePort {
    fn append_comment(&self, req: AppendCommentReq) -> Result<AppendCommentRes, StoreError>;
    fn create_agent(&self, req: CreateAgentReq) -> Result<CreateAgentRes, StoreError>;
    fn create_company(&self, req: CreateCompanyReq) -> Result<CreateCompanyRes, StoreError>;
    fn create_work(&self, req: CreateWorkReq) -> Result<CreateWorkRes, StoreError>;
    fn set_agent_status(&self, req: SetAgentStatusReq) -> Result<SetAgentStatusRes, StoreError>;
    fn update_work(&self, req: UpdateWorkReq) -> Result<UpdateWorkRes, StoreError>;
    fn create_contract_draft(
        &self,
        req: CreateContractDraftReq,
    ) -> Result<CreateContractDraftRes, StoreError>;
    fn activate_contract(
        &self,
        req: ActivateContractReq,
    ) -> Result<ActivateContractRes, StoreError>;
    fn load_context(&self, work_id: &WorkId) -> Result<WorkContext, StoreError>;
    fn merge_wake(&self, req: MergeWakeReq) -> Result<PendingWake, StoreError>;
    fn reap_timed_out_runs(&self, timeout: Duration) -> Result<Vec<ReapedRun>, StoreError>;
    fn load_queued_runs(&self) -> Result<Vec<QueuedRunCandidate>, StoreError>;
    fn load_runtime_turn(&self, run_id: &RunId) -> Result<RuntimeTurnContext, StoreError>;
    fn load_agent_facts(&self, agent_id: &AgentId) -> Result<Option<AgentFacts>, StoreError>;
    fn mark_run_running(&self, run_id: &RunId) -> Result<(), StoreError>;
    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError>;
    fn save_session(&self, session: &TaskSession) -> Result<(), StoreError>;
    fn record_consumption(&self, req: RecordConsumptionReq) -> Result<(), StoreError>;
    fn commit_decision(&self, req: CommitDecisionReq) -> Result<CommitDecisionRes, StoreError>;
    fn read_board(&self) -> BoardReadModel;
    fn read_companies(&self) -> CompanyReadModel;
    fn read_work(&self, work_id: Option<&WorkId>) -> Result<WorkReadModel, StoreError>;
    fn read_agents(&self) -> AgentReadModel;
    fn read_activity(&self) -> ActivityReadModel;
    fn read_run(&self, run_id: &RunId) -> Result<RunReadModel, StoreError>;
    fn read_contracts(&self) -> ContractsReadModel;
}

pub(crate) trait CommandStorePort {
    fn append_comment(&self, req: AppendCommentReq) -> Result<AppendCommentRes, StoreError>;
    fn create_agent(&self, req: CreateAgentReq) -> Result<CreateAgentRes, StoreError>;
    fn create_company(&self, req: CreateCompanyReq) -> Result<CreateCompanyRes, StoreError>;
    fn create_work(&self, req: CreateWorkReq) -> Result<CreateWorkRes, StoreError>;
    fn set_agent_status(&self, req: SetAgentStatusReq) -> Result<SetAgentStatusRes, StoreError>;
    fn update_work(&self, req: UpdateWorkReq) -> Result<UpdateWorkRes, StoreError>;
    fn create_contract_draft(
        &self,
        req: CreateContractDraftReq,
    ) -> Result<CreateContractDraftRes, StoreError>;
    fn activate_contract(
        &self,
        req: ActivateContractReq,
    ) -> Result<ActivateContractRes, StoreError>;
    fn load_context(&self, work_id: &WorkId) -> Result<WorkContext, StoreError>;
    fn load_agent_facts(&self, agent_id: &AgentId) -> Result<Option<AgentFacts>, StoreError>;
    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError>;
    fn commit_decision(&self, req: CommitDecisionReq) -> Result<CommitDecisionRes, StoreError>;
    fn merge_wake(&self, req: MergeWakeReq) -> Result<PendingWake, StoreError>;
}

impl<T> CommandStorePort for T
where
    T: StorePort + ?Sized,
{
    fn append_comment(&self, req: AppendCommentReq) -> Result<AppendCommentRes, StoreError> {
        StorePort::append_comment(self, req)
    }

    fn create_agent(&self, req: CreateAgentReq) -> Result<CreateAgentRes, StoreError> {
        StorePort::create_agent(self, req)
    }

    fn create_company(&self, req: CreateCompanyReq) -> Result<CreateCompanyRes, StoreError> {
        StorePort::create_company(self, req)
    }

    fn create_work(&self, req: CreateWorkReq) -> Result<CreateWorkRes, StoreError> {
        StorePort::create_work(self, req)
    }

    fn set_agent_status(&self, req: SetAgentStatusReq) -> Result<SetAgentStatusRes, StoreError> {
        StorePort::set_agent_status(self, req)
    }

    fn update_work(&self, req: UpdateWorkReq) -> Result<UpdateWorkRes, StoreError> {
        StorePort::update_work(self, req)
    }

    fn create_contract_draft(
        &self,
        req: CreateContractDraftReq,
    ) -> Result<CreateContractDraftRes, StoreError> {
        StorePort::create_contract_draft(self, req)
    }

    fn activate_contract(
        &self,
        req: ActivateContractReq,
    ) -> Result<ActivateContractRes, StoreError> {
        StorePort::activate_contract(self, req)
    }

    fn load_context(&self, work_id: &WorkId) -> Result<WorkContext, StoreError> {
        StorePort::load_context(self, work_id)
    }

    fn load_agent_facts(&self, agent_id: &AgentId) -> Result<Option<AgentFacts>, StoreError> {
        StorePort::load_agent_facts(self, agent_id)
    }

    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError> {
        StorePort::load_session(self, key)
    }

    fn commit_decision(&self, req: CommitDecisionReq) -> Result<CommitDecisionRes, StoreError> {
        StorePort::commit_decision(self, req)
    }

    fn merge_wake(&self, req: MergeWakeReq) -> Result<PendingWake, StoreError> {
        StorePort::merge_wake(self, req)
    }
}

pub(crate) trait RuntimeStorePort {
    fn load_runtime_turn(&self, run_id: &RunId) -> Result<RuntimeTurnContext, StoreError>;
    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError>;
    fn save_session(&self, session: &TaskSession) -> Result<(), StoreError>;
    fn mark_run_running(&self, run_id: &RunId) -> Result<(), StoreError>;
    fn record_consumption(&self, req: RecordConsumptionReq) -> Result<(), StoreError>;
}

impl<T> RuntimeStorePort for T
where
    T: StorePort + ?Sized,
{
    fn load_runtime_turn(&self, run_id: &RunId) -> Result<RuntimeTurnContext, StoreError> {
        StorePort::load_runtime_turn(self, run_id)
    }

    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError> {
        StorePort::load_session(self, key)
    }

    fn save_session(&self, session: &TaskSession) -> Result<(), StoreError> {
        StorePort::save_session(self, session)
    }

    fn mark_run_running(&self, run_id: &RunId) -> Result<(), StoreError> {
        StorePort::mark_run_running(self, run_id)
    }

    fn record_consumption(&self, req: RecordConsumptionReq) -> Result<(), StoreError> {
        StorePort::record_consumption(self, req)
    }
}

pub(crate) trait SchedulerStorePort {
    fn reap_timed_out_runs(&self, timeout: Duration) -> Result<Vec<ReapedRun>, StoreError>;
    fn load_queued_runs(&self) -> Result<Vec<QueuedRunCandidate>, StoreError>;
}

impl<T> SchedulerStorePort for T
where
    T: StorePort + ?Sized,
{
    fn reap_timed_out_runs(&self, timeout: Duration) -> Result<Vec<ReapedRun>, StoreError> {
        StorePort::reap_timed_out_runs(self, timeout)
    }

    fn load_queued_runs(&self) -> Result<Vec<QueuedRunCandidate>, StoreError> {
        StorePort::load_queued_runs(self)
    }
}

pub(crate) trait QueryStorePort {
    fn read_board(&self) -> BoardReadModel;
    fn read_companies(&self) -> CompanyReadModel;
    fn read_work(&self, work_id: Option<&WorkId>) -> Result<WorkReadModel, StoreError>;
    fn read_agents(&self) -> AgentReadModel;
    fn read_activity(&self) -> ActivityReadModel;
    fn read_run(&self, run_id: &RunId) -> Result<RunReadModel, StoreError>;
    fn read_contracts(&self) -> ContractsReadModel;
}

impl<T> QueryStorePort for T
where
    T: StorePort + ?Sized,
{
    fn read_board(&self) -> BoardReadModel {
        StorePort::read_board(self)
    }

    fn read_companies(&self) -> CompanyReadModel {
        StorePort::read_companies(self)
    }

    fn read_work(&self, work_id: Option<&WorkId>) -> Result<WorkReadModel, StoreError> {
        StorePort::read_work(self, work_id)
    }

    fn read_agents(&self) -> AgentReadModel {
        StorePort::read_agents(self)
    }

    fn read_activity(&self) -> ActivityReadModel {
        StorePort::read_activity(self)
    }

    fn read_run(&self, run_id: &RunId) -> Result<RunReadModel, StoreError> {
        StorePort::read_run(self, run_id)
    }

    fn read_contracts(&self) -> ContractsReadModel {
        StorePort::read_contracts(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BoardReadModel {
    pub(crate) running_agents: Vec<String>,
    pub(crate) running_runs: Vec<RunningRunView>,
    pub(crate) pending_wakes: Vec<String>,
    pub(crate) pending_wake_details: Vec<PendingWakeSummaryView>,
    pub(crate) blocked_work: Vec<String>,
    pub(crate) recent_transition_records: Vec<String>,
    pub(crate) recent_transition_details: Vec<BoardTransitionView>,
    pub(crate) recent_gate_failures: Vec<String>,
    pub(crate) recent_gate_failure_details: Vec<BoardGateFailureView>,
    pub(crate) consumption_summary: ConsumptionSummaryView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RunningRunView {
    pub(crate) run_id: String,
    pub(crate) agent_id: String,
    pub(crate) work_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lease_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PendingWakeSummaryView {
    pub(crate) work_id: String,
    pub(crate) count: u32,
    pub(crate) latest_reason: String,
    pub(crate) obligations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BoardTransitionView {
    pub(crate) record_id: String,
    pub(crate) work_id: String,
    pub(crate) kind: String,
    pub(crate) outcome: String,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BoardGateFailureView {
    pub(crate) record_id: String,
    pub(crate) work_id: String,
    pub(crate) outcome: String,
    pub(crate) failed_gates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WorkReadModel {
    pub(crate) items: Vec<WorkSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CompanyReadModel {
    pub(crate) items: Vec<CompanySummaryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CompanySummaryView {
    pub(crate) company_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active_contract_set_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active_contract_revision: Option<u32>,
    pub(crate) agent_count: usize,
    pub(crate) work_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WorkSummary {
    pub(crate) work_id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) kind: crate::model::WorkKind,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) status: WorkStatus,
    pub(crate) rev: u64,
    pub(crate) active_lease_id: Option<String>,
    pub(crate) contract_set_id: String,
    pub(crate) contract_rev: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) contract_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) contract_status: Option<crate::model::ContractSetStatus>,
    pub(crate) pending_obligations: Vec<String>,
    pub(crate) comments: Vec<WorkCommentView>,
    pub(crate) audit_entries: Vec<ActivityEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WorkCommentView {
    pub(crate) author_kind: ActorKind,
    pub(crate) author_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<String>,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentReadModel {
    pub(crate) active_agents: Vec<String>,
    pub(crate) registered_agents: Vec<AgentSummaryView>,
    pub(crate) recent_runs: Vec<AgentRunView>,
    pub(crate) current_sessions: Vec<AgentSessionSummaryView>,
    pub(crate) consumption_by_agent: Vec<AgentConsumptionSummaryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentSummaryView {
    pub(crate) agent_id: String,
    pub(crate) company_id: String,
    pub(crate) name: String,
    pub(crate) role: String,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentFacts {
    pub(crate) company_id: CompanyId,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentRunView {
    pub(crate) run_id: String,
    pub(crate) agent_id: String,
    pub(crate) work_id: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentSessionSummaryView {
    pub(crate) agent_id: String,
    pub(crate) work_id: String,
    pub(crate) runtime: RuntimeKind,
    pub(crate) runtime_session_id: String,
    pub(crate) cwd: String,
    pub(crate) contract_rev: u32,
    pub(crate) last_decision_summary: Option<String>,
    pub(crate) last_gate_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ConsumptionSummaryView {
    pub(crate) total_turns: u64,
    pub(crate) total_input_tokens: u64,
    pub(crate) total_output_tokens: u64,
    pub(crate) total_run_seconds: u64,
    pub(crate) total_estimated_cost_cents: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentConsumptionSummaryView {
    pub(crate) agent_id: String,
    pub(crate) total_turns: u64,
    pub(crate) total_input_tokens: u64,
    pub(crate) total_output_tokens: u64,
    pub(crate) total_run_seconds: u64,
    pub(crate) total_estimated_cost_cents: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ActivityReadModel {
    pub(crate) entries: Vec<ActivityEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RunReadModel {
    pub(crate) run_id: String,
    pub(crate) agent_id: String,
    pub(crate) work_id: String,
    pub(crate) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_session: Option<AgentSessionSummaryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ActivityEntryView {
    pub(crate) event_kind: String,
    pub(crate) work_id: String,
    pub(crate) summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) actor_kind: Option<ActorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) actor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<String>,
    pub(crate) before_status: Option<WorkStatus>,
    pub(crate) after_status: Option<WorkStatus>,
    pub(crate) outcome: Option<String>,
    pub(crate) evidence_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ContractsReadModel {
    pub(crate) contract_set_id: String,
    pub(crate) name: String,
    pub(crate) revision: u32,
    pub(crate) status: ContractSetStatus,
    pub(crate) revisions: Vec<ContractRevisionView>,
    pub(crate) rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ContractRevisionView {
    pub(crate) revision: u32,
    pub(crate) status: ContractSetStatus,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimLeaseReq {
    pub(crate) work_id: WorkId,
    pub(crate) agent_id: AgentId,
    pub(crate) lease_id: LeaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppendCommentReq {
    pub(crate) company_id: CompanyId,
    pub(crate) work_id: WorkId,
    pub(crate) author_kind: ActorKind,
    pub(crate) author_id: ActorId,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppendCommentRes {
    pub(crate) comment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateContractDraftReq {
    pub(crate) company_id: CompanyId,
    pub(crate) name: String,
    pub(crate) rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateAgentReq {
    pub(crate) company_id: CompanyId,
    pub(crate) name: String,
    pub(crate) role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateCompanyReq {
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateCompanyRes {
    pub(crate) profile: CompanyProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateAgentRes {
    pub(crate) agent_id: AgentId,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateWorkReq {
    pub(crate) company_id: CompanyId,
    pub(crate) parent_id: Option<WorkId>,
    pub(crate) kind: crate::model::WorkKind,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) contract_set_id: crate::model::ContractSetId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateWorkRes {
    pub(crate) snapshot: WorkSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetAgentStatusReq {
    pub(crate) agent_id: AgentId,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetAgentStatusRes {
    pub(crate) agent_id: AgentId,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateWorkReq {
    pub(crate) work_id: WorkId,
    pub(crate) parent_id: Option<WorkId>,
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateWorkRes {
    pub(crate) snapshot: WorkSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateContractDraftRes {
    pub(crate) revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivateContractReq {
    pub(crate) company_id: CompanyId,
    pub(crate) revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivateContractRes {
    pub(crate) revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimLeaseRes {
    pub(crate) lease: WorkLease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkContext {
    pub(crate) snapshot: WorkSnapshot,
    pub(crate) lease: Option<WorkLease>,
    pub(crate) pending_wake: Option<PendingWake>,
    pub(crate) contract: ContractSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeWakeReq {
    pub(crate) work_id: WorkId,
    pub(crate) actor_kind: ActorKind,
    pub(crate) actor_id: ActorId,
    pub(crate) source: String,
    pub(crate) reason: String,
    pub(crate) obligations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReapedRun {
    pub(crate) run_id: RunId,
    pub(crate) work_id: WorkId,
    pub(crate) released_lease_id: Option<LeaseId>,
    pub(crate) follow_up_run_id: Option<RunId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueuedRunCandidate {
    pub(crate) run_id: RunId,
    pub(crate) agent_status: Option<AgentStatus>,
    pub(crate) created_at: crate::model::Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeTurnContext {
    pub(crate) run_id: RunId,
    pub(crate) agent_id: AgentId,
    pub(crate) snapshot: WorkSnapshot,
    pub(crate) pending_wake: Option<PendingWake>,
    pub(crate) contract: ContractSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionKey {
    pub(crate) agent_id: AgentId,
    pub(crate) work_id: WorkId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordConsumptionReq {
    pub(crate) company_id: CompanyId,
    pub(crate) agent_id: AgentId,
    pub(crate) run_id: RunId,
    pub(crate) billing_kind: BillingKind,
    pub(crate) usage: ConsumptionUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitDecisionReq {
    pub(crate) decision: TransitionDecision,
    pub(crate) record: TransitionRecord,
    pub(crate) session: Option<TaskSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitDecisionRes {
    pub(crate) snapshot: Option<WorkSnapshot>,
    pub(crate) lease: Option<WorkLease>,
    pub(crate) pending_wake: Option<PendingWake>,
    pub(crate) session: Option<TaskSession>,
    pub(crate) activity_event: Option<ActivityEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreError {
    pub(crate) kind: StoreErrorKind,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreErrorKind {
    Conflict,
    NotFound,
    Unavailable,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl StoreError {
    pub(crate) fn unavailable(message: String) -> Self {
        Self {
            kind: StoreErrorKind::Unavailable,
            message,
        }
    }
}

impl Error for StoreError {}
