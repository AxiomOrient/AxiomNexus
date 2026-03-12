use serde::{Deserialize, Serialize};

use super::{AgentId, CompanyId, ContractRev, ContractSetId, LeaseId, Rev, Timestamp, WorkId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkSnapshot {
    pub(crate) work_id: WorkId,
    pub(crate) company_id: CompanyId,
    pub(crate) parent_id: Option<WorkId>,
    pub(crate) kind: WorkKind,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) status: WorkStatus,
    pub(crate) priority: Priority,
    pub(crate) assignee_agent_id: Option<AgentId>,
    pub(crate) active_lease_id: Option<LeaseId>,
    pub(crate) rev: Rev,
    pub(crate) contract_set_id: ContractSetId,
    pub(crate) contract_rev: ContractRev,
    pub(crate) created_at: Timestamp,
    pub(crate) updated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkKind {
    Objective,
    Project,
    Task,
    Decision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkStatus {
    Backlog,
    Todo,
    Doing,
    Blocked,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Priority {
    Critical,
    High,
    Medium,
    Low,
}
