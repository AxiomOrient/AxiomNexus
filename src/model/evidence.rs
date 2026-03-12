use serde::{Deserialize, Serialize};

use super::{AgentStatus, CompanyId, GateSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct EvidenceBundle {
    pub(crate) changed_files: Vec<FileChange>,
    pub(crate) command_results: Vec<CommandResult>,
    pub(crate) gate_results: Vec<GateResult>,
    pub(crate) artifact_refs: Vec<EvidenceRef>,
    pub(crate) observed_agent_status: Option<AgentStatus>,
    pub(crate) observed_agent_company_id: Option<CompanyId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileChange {
    pub(crate) path: String,
    pub(crate) change_kind: ChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CommandResult {
    pub(crate) argv: Vec<String>,
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) failure_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GateResult {
    pub(crate) gate: GateSpec,
    pub(crate) passed: bool,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceRef {
    pub(crate) kind: EvidenceRefKind,
    pub(crate) location: String,
    pub(crate) digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceRefKind {
    Artifact,
    Blob,
    Log,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceInline {
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReasonCode {
    LeaseConflict,
    RevConflict,
    ContractDenied,
    GateFailed,
    NoteMissing,
    SchemaInvalid,
    StaleLease,
}
