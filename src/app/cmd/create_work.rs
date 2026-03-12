use serde::Serialize;

use crate::{
    model::{CompanyId, ContractSetId, WorkId, WorkKind, WorkStatus},
    port::store::{CommandStorePort, CreateWorkReq, StoreError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateWorkCmd {
    pub(crate) company_id: CompanyId,
    pub(crate) parent_id: Option<WorkId>,
    pub(crate) kind: WorkKind,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) contract_set_id: ContractSetId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreateWorkAck {
    pub(crate) work_id: String,
    pub(crate) status: WorkStatus,
}

pub(crate) fn handle_create_work(
    store: &impl CommandStorePort,
    cmd: CreateWorkCmd,
) -> Result<CreateWorkAck, StoreError> {
    let created = store.create_work(CreateWorkReq {
        company_id: cmd.company_id,
        parent_id: cmd.parent_id,
        kind: cmd.kind,
        title: cmd.title,
        body: cmd.body,
        contract_set_id: cmd.contract_set_id,
    })?;

    Ok(CreateWorkAck {
        work_id: created.snapshot.work_id.to_string(),
        status: created.snapshot.status,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{
            MemoryStore, DEMO_COMPANY_ID, DEMO_CONTRACT_SET_ID, DEMO_TODO_WORK_ID,
        },
        model::{CompanyId, ContractSetId, WorkId, WorkKind, WorkStatus},
        port::store::StorePort,
    };

    use super::{handle_create_work, CreateWorkCmd};

    #[test]
    fn create_work_creates_backlog_item_under_parent() {
        let store = MemoryStore::demo();

        let ack = handle_create_work(
            &store,
            CreateWorkCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                parent_id: Some(WorkId::from(DEMO_TODO_WORK_ID)),
                kind: WorkKind::Decision,
                title: "Decide release".to_owned(),
                body: "Compare rollout options".to_owned(),
                contract_set_id: ContractSetId::from(DEMO_CONTRACT_SET_ID),
            },
        )
        .expect("create work should succeed");

        let work = store
            .read_work(Some(&WorkId::from(ack.work_id.as_str())))
            .expect("created work should be readable");

        assert!(!ack.work_id.contains("0x"));
        assert_eq!(ack.status, WorkStatus::Backlog);
        assert_eq!(work.items[0].parent_id.as_deref(), Some(DEMO_TODO_WORK_ID));
        assert_eq!(work.items[0].title, "Decide release");
        assert_eq!(work.items[0].kind, WorkKind::Decision);
    }
}
