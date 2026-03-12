use serde::{Deserialize, Serialize};

use super::{AgentId, CompanyId, ContractRev, RecordId, SessionId, Timestamp, WorkId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TaskSession {
    pub(crate) session_id: SessionId,
    pub(crate) company_id: CompanyId,
    pub(crate) agent_id: AgentId,
    pub(crate) work_id: WorkId,
    pub(crate) runtime: RuntimeKind,
    pub(crate) runtime_session_id: String,
    pub(crate) cwd: String,
    pub(crate) contract_rev: ContractRev,
    pub(crate) last_record_id: Option<RecordId>,
    pub(crate) last_decision_summary: Option<String>,
    pub(crate) last_gate_summary: Option<String>,
    pub(crate) updated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeKind {
    Coclai,
}
