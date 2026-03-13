use std::{error::Error, fmt};

use crate::model::{
    ConsumptionUsage, SessionInvalidationReason, TaskSession, TransitionIntent, WorkSnapshot,
};

use crate::port::store::SessionKey;

pub(crate) trait RuntimePort {
    fn execute_turn(&self, req: ExecuteTurnReq) -> Result<ExecuteTurnOutcome, RuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptEnvelopeInput {
    pub(crate) snapshot: WorkSnapshot,
    pub(crate) unresolved_obligations: Vec<String>,
    pub(crate) contract_summary: String,
    pub(crate) last_gate_summary: Option<String>,
    pub(crate) last_decision_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecuteTurnReq {
    pub(crate) session_key: SessionKey,
    pub(crate) cwd: String,
    pub(crate) existing_session: Option<TaskSession>,
    pub(crate) prompt_input: PromptEnvelopeInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecuteTurnOutcome {
    pub(crate) handle: RuntimeHandle,
    pub(crate) result: RuntimeResult,
    pub(crate) resumed: bool,
    pub(crate) repair_count: u8,
    pub(crate) session_reset_reason: Option<SessionInvalidationReason>,
    pub(crate) prompt_envelope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeHandle {
    pub(crate) runtime_session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeResult {
    pub(crate) intent: TransitionIntent,
    pub(crate) raw_output: String,
    pub(crate) usage: ConsumptionUsage,
    pub(crate) invalid_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeError {
    pub(crate) kind: RuntimeErrorKind,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeErrorKind {
    Transport,
    InvalidSession,
    InvalidOutput,
    Unavailable,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for RuntimeError {}
