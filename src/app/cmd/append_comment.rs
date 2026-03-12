use crate::{
    model::{ActorId, ActorKind, CompanyId, WorkId},
    port::store::{AppendCommentReq, CommandStorePort, StoreError},
};

use super::COMMENTS_TABLE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppendCommentCmd {
    pub(crate) company_id: CompanyId,
    pub(crate) work_id: WorkId,
    pub(crate) author_kind: ActorKind,
    pub(crate) author_id: ActorId,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppendCommentAck {
    pub(crate) persisted_table: &'static str,
    pub(crate) comment_id: String,
}

pub(crate) fn handle_append_comment(
    store: &impl CommandStorePort,
    cmd: AppendCommentCmd,
) -> Result<AppendCommentAck, StoreError> {
    let persisted = store.append_comment(AppendCommentReq {
        company_id: cmd.company_id,
        work_id: cmd.work_id,
        author_kind: cmd.author_kind,
        author_id: cmd.author_id,
        body: cmd.body,
    })?;

    Ok(AppendCommentAck {
        persisted_table: COMMENTS_TABLE,
        comment_id: persisted.comment_id,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_COMPANY_ID, DEMO_TODO_WORK_ID},
        model::{ActorId, ActorKind, CompanyId, WorkId},
        port::store::StorePort,
    };

    use super::{handle_append_comment, AppendCommentCmd};

    #[test]
    fn append_comment_persists_and_surfaces_on_work_read() {
        let store = MemoryStore::demo();

        let ack = handle_append_comment(
            &store,
            AppendCommentCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                author_kind: ActorKind::Board,
                author_id: ActorId::from("00000000-0000-4000-8000-000000000031"),
                body: "board note".to_owned(),
            },
        )
        .expect("comment append should persist");

        let work = store
            .read_work(Some(&WorkId::from(DEMO_TODO_WORK_ID)))
            .expect("work should be readable");

        assert_eq!(ack.persisted_table, "work_comments");
        assert!(ack.comment_id.starts_with("comment-"));
        assert_eq!(work.items[0].comments.len(), 1);
        assert_eq!(work.items[0].comments[0].body, "board note");
        assert_eq!(work.items[0].comments[0].author_kind, ActorKind::Board);
    }
}
