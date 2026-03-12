use serde::Serialize;

use crate::{
    model::{CompanyId, TransitionRule},
    port::store::{CommandStorePort, CreateContractDraftReq, StoreError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateContractDraftCmd {
    pub(crate) company_id: CompanyId,
    pub(crate) name: String,
    pub(crate) rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreateContractDraftAck {
    pub(crate) revision: u32,
}

pub(crate) fn handle_create_contract_draft(
    store: &impl CommandStorePort,
    cmd: CreateContractDraftCmd,
) -> Result<CreateContractDraftAck, StoreError> {
    let created = store.create_contract_draft(CreateContractDraftReq {
        company_id: cmd.company_id,
        name: cmd.name,
        rules: cmd.rules,
    })?;

    Ok(CreateContractDraftAck {
        revision: created.revision,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_COMPANY_ID},
        model::{CompanyId, TransitionKind},
        port::store::StorePort,
    };

    use super::{handle_create_contract_draft, CreateContractDraftCmd};

    #[test]
    fn create_contract_draft_appends_new_draft_revision() {
        let store = MemoryStore::demo();
        let rules = store.read_contracts().rules;

        let ack = handle_create_contract_draft(
            &store,
            CreateContractDraftCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                name: "axiomnexus-rust-next".to_owned(),
                rules,
            },
        )
        .expect("draft create should succeed");

        let contracts = store.read_contracts();

        assert_eq!(ack.revision, 3);
        assert_eq!(contracts.revisions.len(), 4);
        assert_eq!(
            contracts.revisions.last().expect("draft revision").revision,
            3
        );
        assert_eq!(
            contracts
                .rules
                .iter()
                .filter(|rule| rule.kind == TransitionKind::Block)
                .count(),
            1
        );
    }
}
