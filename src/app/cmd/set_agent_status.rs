use serde::Serialize;

use crate::{
    model::{AgentId, AgentStatus},
    port::store::{CommandStorePort, SetAgentStatusReq, StoreError},
};

use super::AGENT_STATUS_POLICY;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetAgentStatusCmd {
    pub(crate) agent_id: AgentId,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SetAgentStatusAck {
    pub(crate) policy: &'static str,
    pub(crate) agent_id: String,
    pub(crate) status: AgentStatus,
}

pub(crate) fn handle_set_agent_status(
    store: &impl CommandStorePort,
    cmd: SetAgentStatusCmd,
) -> Result<SetAgentStatusAck, StoreError> {
    let updated = store.set_agent_status(SetAgentStatusReq {
        agent_id: cmd.agent_id,
        status: cmd.status,
    })?;

    Ok(SetAgentStatusAck {
        policy: AGENT_STATUS_POLICY,
        agent_id: updated.agent_id.to_string(),
        status: updated.status,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{
            MemoryStore, DEMO_AGENT_ID, DEMO_TERMINATED_AGENT_ID, DEMO_TODO_WORK_ID,
        },
        app::cmd::run_scheduler::next_queued_run_id,
        model::{AgentId, AgentStatus, WorkId},
        port::store::{MergeWakeReq, StorePort},
    };

    use super::{handle_set_agent_status, SetAgentStatusCmd};

    #[test]
    fn pause_agent_removes_it_from_scheduler_pick_until_resumed() {
        let store = MemoryStore::demo();
        store
            .merge_wake(MergeWakeReq {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: crate::model::ActorKind::Board,
                actor_id: crate::model::ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "agent lifecycle".to_owned(),
                obligations: vec!["follow up".to_owned()],
            })
            .expect("wake should create queued run");

        handle_set_agent_status(
            &store,
            SetAgentStatusCmd {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                status: AgentStatus::Paused,
            },
        )
        .expect("pause should succeed");
        assert!(next_queued_run_id(
            &store
                .load_queued_runs()
                .expect("queued run lookup should work")
        )
        .is_none());

        let resumed = handle_set_agent_status(
            &store,
            SetAgentStatusCmd {
                agent_id: AgentId::from(DEMO_AGENT_ID),
                status: AgentStatus::Active,
            },
        )
        .expect("resume should succeed");

        assert_eq!(resumed.status, AgentStatus::Active);
        assert_eq!(
            next_queued_run_id(
                &store
                    .load_queued_runs()
                    .expect("queued run should become visible again")
            )
            .as_ref()
            .map(|run_id| run_id.as_str()),
            Some("run-2")
        );
    }

    #[test]
    fn terminated_agent_cannot_resume() {
        let store = MemoryStore::demo();

        let error = handle_set_agent_status(
            &store,
            SetAgentStatusCmd {
                agent_id: AgentId::from(DEMO_TERMINATED_AGENT_ID),
                status: AgentStatus::Active,
            },
        )
        .expect_err("terminated agent resume should conflict");

        assert!(error.message.contains("terminated"));
    }
}
