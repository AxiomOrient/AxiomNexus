use serde::{Deserialize, Serialize};

use super::{CompanyId, ContractRev, ContractSetId, LeaseEffect, TransitionKind, WorkStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ContractSet {
    pub(crate) contract_set_id: ContractSetId,
    pub(crate) company_id: CompanyId,
    pub(crate) revision: ContractRev,
    pub(crate) name: String,
    pub(crate) status: ContractSetStatus,
    pub(crate) rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContractSetStatus {
    Draft,
    Active,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TransitionRule {
    pub(crate) kind: TransitionKind,
    pub(crate) actor_kind: ActorKind,
    pub(crate) from: Vec<WorkStatus>,
    pub(crate) to: WorkStatus,
    pub(crate) lease_effect: LeaseEffect,
    pub(crate) gates: Vec<GateSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActorKind {
    Agent,
    Board,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub(crate) enum GateSpec {
    #[serde(rename = "NoOpenLease")]
    NoOpenLease,
    #[serde(rename = "AgentIsRunnable")]
    AgentIsRunnable,
    #[serde(rename = "LeasePresent")]
    LeasePresent,
    #[serde(rename = "LeaseHeldByActor")]
    LeaseHeldByActor,
    #[serde(rename = "ExpectedRevMatchesSnapshot")]
    ExpectedRevMatchesSnapshot,
    #[serde(rename = "SummaryPresent")]
    SummaryPresent,
    #[serde(rename = "ManualNotePresent")]
    ManualNotePresent,
    #[serde(rename = "ChangedFilesObserved")]
    ChangedFilesObserved,
    #[serde(rename = "AllRequiredObligationsResolved")]
    AllRequiredObligationsResolved,
    #[serde(rename = "CommandSucceeds")]
    CommandSucceeds {
        argv: Vec<String>,
        timeout_sec: u64,
        allow_exit_codes: Vec<i32>,
    },
}
