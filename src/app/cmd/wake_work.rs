use serde::Serialize;

use crate::{
    model::{ActorId, ActorKind, WorkId},
    port::store::{CommandStorePort, MergeWakeReq, StoreError},
};

use super::WAKE_QUEUE_POLICY;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WakeWorkCmd {
    pub(crate) work_id: WorkId,
    pub(crate) actor_kind: ActorKind,
    pub(crate) actor_id: ActorId,
    pub(crate) source: String,
    pub(crate) latest_reason: String,
    pub(crate) obligation_delta: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WakeWorkAck {
    pub(crate) queue_policy: &'static str,
    pub(crate) merged_count: u32,
}

pub(crate) fn handle_wake_work(
    store: &impl CommandStorePort,
    cmd: WakeWorkCmd,
) -> Result<WakeWorkAck, StoreError> {
    let wake = store.merge_wake(MergeWakeReq {
        work_id: cmd.work_id,
        actor_kind: cmd.actor_kind,
        actor_id: cmd.actor_id,
        source: cmd.source,
        reason: cmd.latest_reason,
        obligations: cmd.obligation_delta,
    })?;

    Ok(WakeWorkAck {
        queue_policy: WAKE_QUEUE_POLICY,
        merged_count: wake.count,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_TODO_WORK_ID},
        model::{ActorId, ActorKind, WorkId},
        port::store::StorePort,
    };

    use super::{handle_wake_work, WakeWorkCmd};

    #[test]
    fn wake_work_persists_actor_and_source_trace() {
        let store = MemoryStore::demo();

        let ack = handle_wake_work(
            &store,
            WakeWorkCmd {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("board"),
                source: "manual".to_owned(),
                latest_reason: "gate failed".to_owned(),
                obligation_delta: vec!["cargo test".to_owned()],
            },
        )
        .expect("wake work should succeed");

        let work = store
            .read_work(Some(&WorkId::from(DEMO_TODO_WORK_ID)))
            .expect("work should be readable");

        assert_eq!(ack.merged_count, 1);
        assert_eq!(work.items[0].comments.len(), 1);
        assert_eq!(work.items[0].comments[0].author_kind, ActorKind::Board);
        assert_eq!(work.items[0].comments[0].author_id, "board");
        assert_eq!(work.items[0].comments[0].source.as_deref(), Some("manual"));
        assert!(work.items[0].comments[0]
            .body
            .contains("manual wake merged"));
        assert_eq!(
            work.items[0].audit_entries[0].actor_kind,
            Some(ActorKind::Board)
        );
        assert_eq!(
            work.items[0].audit_entries[0].actor_id.as_deref(),
            Some("board")
        );
        assert_eq!(
            work.items[0].audit_entries[0].source.as_deref(),
            Some("manual")
        );
    }
}
