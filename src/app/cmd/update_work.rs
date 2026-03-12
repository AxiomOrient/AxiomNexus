use serde::Serialize;

use crate::{
    model::{WorkId, WorkStatus},
    port::store::{CommandStorePort, StoreError, UpdateWorkReq},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateWorkCmd {
    pub(crate) work_id: WorkId,
    pub(crate) parent_id: Option<WorkId>,
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UpdateWorkAck {
    pub(crate) work_id: String,
    pub(crate) status: WorkStatus,
    pub(crate) rev: u64,
}

pub(crate) fn handle_update_work(
    store: &impl CommandStorePort,
    cmd: UpdateWorkCmd,
) -> Result<UpdateWorkAck, StoreError> {
    let updated = store.update_work(UpdateWorkReq {
        work_id: cmd.work_id,
        parent_id: cmd.parent_id,
        title: cmd.title,
        body: cmd.body,
    })?;

    Ok(UpdateWorkAck {
        work_id: updated.snapshot.work_id.to_string(),
        status: updated.snapshot.status,
        rev: updated.snapshot.rev,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_DOING_WORK_ID},
        model::WorkId,
        port::store::StorePort,
    };

    use super::{handle_update_work, UpdateWorkCmd};

    #[test]
    fn update_work_edits_metadata_without_changing_status() {
        let store = MemoryStore::demo();

        let ack = handle_update_work(
            &store,
            UpdateWorkCmd {
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                parent_id: None,
                title: "Doing work updated".to_owned(),
                body: "new body".to_owned(),
            },
        )
        .expect("update work should succeed");

        let work = store
            .read_work(Some(&WorkId::from(DEMO_DOING_WORK_ID)))
            .expect("updated work should be readable");

        assert_eq!(ack.work_id, DEMO_DOING_WORK_ID);
        assert_eq!(work.items[0].title, "Doing work updated");
        assert_eq!(work.items[0].body, "new body");
        assert_eq!(work.items[0].status, ack.status);
        assert_eq!(ack.rev, 2);
    }
}
