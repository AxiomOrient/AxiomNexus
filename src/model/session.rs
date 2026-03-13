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
    pub(crate) workspace_fingerprint: String,
    pub(crate) contract_rev: ContractRev,
    pub(crate) last_record_id: Option<RecordId>,
    pub(crate) last_decision_summary: Option<String>,
    pub(crate) last_gate_summary: Option<String>,
    pub(crate) updated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionInvalidationReason {
    Agent,
    Work,
    Workspace,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeKind {
    Coclai,
}

pub(crate) fn workspace_fingerprint(cwd: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in cwd.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
