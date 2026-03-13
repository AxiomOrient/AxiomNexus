use serde::Serialize;

use crate::port::store::{CommandStorePort, CreateCompanyReq, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateCompanyCmd {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) runtime_hard_stop_cents: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreateCompanyAck {
    pub(crate) company_id: String,
}

pub(crate) fn handle_create_company(
    store: &impl CommandStorePort,
    cmd: CreateCompanyCmd,
) -> Result<CreateCompanyAck, StoreError> {
    let created = store.create_company(CreateCompanyReq {
        name: cmd.name,
        description: cmd.description,
        runtime_hard_stop_cents: cmd.runtime_hard_stop_cents,
    })?;

    Ok(CreateCompanyAck {
        company_id: created.profile.company_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use crate::{adapter::memory::store::MemoryStore, port::store::StorePort};

    use super::{handle_create_company, CreateCompanyCmd};

    #[test]
    fn create_company_persists_profile() {
        let store = MemoryStore::demo();

        let ack = handle_create_company(
            &store,
            CreateCompanyCmd {
                name: "Acme".to_owned(),
                description: "release scope".to_owned(),
                runtime_hard_stop_cents: None,
            },
        )
        .expect("company create should succeed");

        let companies = store.read_companies();

        assert!(!ack.company_id.contains("0x"));
        assert!(companies
            .items
            .iter()
            .any(|item| item.company_id == ack.company_id
                && item.name == "Acme"
                && item.description == "release scope"
                && item.agent_count == 0
                && item.work_count == 0
                && item.active_contract_set_id.is_none()
                && item.active_contract_revision.is_none()));
    }
}
