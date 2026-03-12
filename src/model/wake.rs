use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Timestamp, WorkId};

pub(crate) type ObligationSet = BTreeSet<String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PendingWake {
    pub(crate) work_id: WorkId,
    pub(crate) obligation_json: ObligationSet,
    pub(crate) count: u32,
    pub(crate) latest_reason: String,
    pub(crate) merged_at: Timestamp,
}
