use serde::Serialize;

use crate::{
    model::CompanyId,
    port::store::{ActivateContractReq, CommandStorePort, StoreError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivateContractCmd {
    pub(crate) company_id: CompanyId,
    pub(crate) revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ActivateContractAck {
    pub(crate) revision: u32,
}

pub(crate) fn handle_activate_contract(
    store: &impl CommandStorePort,
    cmd: ActivateContractCmd,
) -> Result<ActivateContractAck, StoreError> {
    let activated = store.activate_contract(ActivateContractReq {
        company_id: cmd.company_id,
        revision: cmd.revision,
    })?;

    Ok(ActivateContractAck {
        revision: activated.revision,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_COMPANY_ID},
        app::cmd::create_contract_draft::{handle_create_contract_draft, CreateContractDraftCmd},
        model::{CompanyId, ContractSetStatus},
        port::store::StorePort,
    };

    use super::{handle_activate_contract, ActivateContractCmd};

    #[test]
    fn activate_contract_promotes_selected_revision_and_retires_previous_active() {
        let store = MemoryStore::demo();
        let rules = store.read_contracts().rules;
        handle_create_contract_draft(
            &store,
            CreateContractDraftCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                name: "axiomnexus-rust-next".to_owned(),
                rules,
            },
        )
        .expect("draft create should succeed");

        let ack = handle_activate_contract(
            &store,
            ActivateContractCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                revision: 3,
            },
        )
        .expect("activate should succeed");

        let contracts = store.read_contracts();

        assert_eq!(ack.revision, 3);
        assert_eq!(contracts.revision, 3);
        assert_eq!(contracts.status, ContractSetStatus::Active);
        assert_eq!(contracts.name, "axiomnexus-rust-next");
        assert_eq!(
            contracts
                .revisions
                .iter()
                .filter(|revision| revision.status == ContractSetStatus::Active)
                .count(),
            1
        );
    }
}
