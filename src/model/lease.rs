use serde::{Deserialize, Serialize};

use super::{AgentId, CompanyId, LeaseId, RunId, Timestamp, WorkId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkLease {
    pub(crate) lease_id: LeaseId,
    pub(crate) company_id: CompanyId,
    pub(crate) work_id: WorkId,
    pub(crate) agent_id: AgentId,
    pub(crate) run_id: Option<RunId>,
    pub(crate) acquired_at: Timestamp,
    pub(crate) expires_at: Option<Timestamp>,
    pub(crate) released_at: Option<Timestamp>,
    pub(crate) release_reason: Option<LeaseReleaseReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeaseReleaseReason {
    Completed,
    Blocked,
    Cancelled,
    Overridden,
    Conflict,
    Expired,
}
