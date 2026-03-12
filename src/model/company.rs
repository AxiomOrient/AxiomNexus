use serde::{Deserialize, Serialize};

use super::CompanyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CompanyProfile {
    pub(crate) company_id: CompanyId,
    pub(crate) name: String,
    pub(crate) description: String,
}
