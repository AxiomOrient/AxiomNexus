use serde::Serialize;

use crate::{
    model::{AgentStatus, CompanyId},
    port::store::{CommandStorePort, CreateAgentReq, StoreError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateAgentCmd {
    pub(crate) company_id: CompanyId,
    pub(crate) name: String,
    pub(crate) role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreateAgentAck {
    pub(crate) agent_id: String,
    pub(crate) status: AgentStatus,
}

pub(crate) fn handle_create_agent(
    store: &impl CommandStorePort,
    cmd: CreateAgentCmd,
) -> Result<CreateAgentAck, StoreError> {
    let created = store.create_agent(CreateAgentReq {
        company_id: cmd.company_id,
        name: cmd.name,
        role: cmd.role,
    })?;

    Ok(CreateAgentAck {
        agent_id: created.agent_id.to_string(),
        status: created.status,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        adapter::memory::store::{MemoryStore, DEMO_COMPANY_ID},
        model::{AgentStatus, CompanyId},
        port::store::StorePort,
    };

    use super::{handle_create_agent, CreateAgentCmd};

    #[test]
    fn create_agent_registers_active_agent_with_profile() {
        let store = MemoryStore::demo();

        let ack = handle_create_agent(
            &store,
            CreateAgentCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                name: "Release Operator".to_owned(),
                role: "release_manager".to_owned(),
            },
        )
        .expect("agent create should succeed");

        let agents = store.read_agents();
        let created = agents
            .registered_agents
            .iter()
            .find(|agent| agent.agent_id == ack.agent_id)
            .expect("created agent should be readable");

        assert!(!ack.agent_id.contains("0x"));
        assert_eq!(ack.status, AgentStatus::Active);
        assert_eq!(created.name, "Release Operator");
        assert_eq!(created.role, "release_manager");
        assert_eq!(created.status, AgentStatus::Active);
    }
}
