use serde::{Deserialize, Serialize};

use super::CompanyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CompanyProfile {
    pub(crate) company_id: CompanyId,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) runtime_hard_stop_cents: Option<u64>,
    #[serde(default)]
    pub(crate) recorded_estimated_cost_cents: u64,
}
