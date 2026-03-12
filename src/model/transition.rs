use serde::{Deserialize, Serialize};

use super::{
    ActorId, ActorKind, CompanyId, EvidenceBundle, EvidenceInline, EvidenceRef, LeaseId,
    ReasonCode, Rev, WorkId, WorkSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionIntent {
    pub(crate) work_id: WorkId,
    pub(crate) agent_id: super::AgentId,
    pub(crate) lease_id: LeaseId,
    pub(crate) expected_rev: Rev,
    pub(crate) kind: TransitionKind,
    pub(crate) patch: WorkPatch,
    pub(crate) note: Option<String>,
    pub(crate) proof_hints: Vec<ProofHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkPatch {
    pub(crate) summary: String,
    pub(crate) resolved_obligations: Vec<String>,
    pub(crate) declared_risks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProofHint {
    pub(crate) kind: ProofHintKind,
    pub(crate) value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProofHintKind {
    Summary,
    File,
    Command,
    Artifact,
    Risk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TransitionDecision {
    pub(crate) outcome: DecisionOutcome,
    pub(crate) reasons: Vec<ReasonCode>,
    pub(crate) next_snapshot: Option<WorkSnapshot>,
    pub(crate) lease_effect: LeaseEffect,
    pub(crate) pending_wake_effect: PendingWakeEffect,
    pub(crate) gate_results: Vec<super::GateResult>,
    pub(crate) evidence: EvidenceBundle,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TransitionRecord {
    pub(crate) record_id: super::RecordId,
    pub(crate) company_id: CompanyId,
    pub(crate) work_id: WorkId,
    pub(crate) actor_kind: ActorKind,
    pub(crate) actor_id: ActorId,
    pub(crate) lease_id: Option<LeaseId>,
    pub(crate) expected_rev: Rev,
    pub(crate) before_status: super::WorkStatus,
    pub(crate) after_status: Option<super::WorkStatus>,
    pub(crate) outcome: DecisionOutcome,
    #[serde(default)]
    pub(crate) reasons: Vec<ReasonCode>,
    pub(crate) kind: TransitionKind,
    pub(crate) patch: WorkPatch,
    pub(crate) gate_results: Vec<super::GateResult>,
    #[serde(default)]
    pub(crate) evidence: EvidenceBundle,
    pub(crate) evidence_inline: Option<EvidenceInline>,
    pub(crate) evidence_refs: Vec<EvidenceRef>,
    pub(crate) happened_at: super::Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransitionKind {
    Queue,
    Claim,
    ProposeProgress,
    Complete,
    Block,
    Reopen,
    Cancel,
    OverrideComplete,
    TimeoutRequeue,
}

impl TransitionKind {
    pub(crate) fn is_runtime_intent(self) -> bool {
        matches!(self, Self::ProposeProgress | Self::Complete | Self::Block)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionOutcome {
    Accepted,
    Rejected,
    Conflict,
    OverrideAccepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeaseEffect {
    None,
    Acquire,
    Keep,
    Release,
    Renew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PendingWakeEffect {
    None,
    Retain,
    Clear,
    Merge,
}
