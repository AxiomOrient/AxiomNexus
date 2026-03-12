use serde::Deserialize;

pub(crate) const METHOD_GET: &str = "GET";
pub(crate) const METHOD_POST: &str = "POST";

pub(crate) const BOARD_ROUTE: &str = "/api/board";
pub(crate) const COMPANIES_ROUTE: &str = "/api/companies";
pub(crate) const WORK_COLLECTION_ROUTE: &str = "/api/work";
pub(crate) const WORK_DETAIL_ROUTE: &str = "/api/work/{id}";
pub(crate) const WORK_UPDATE_ROUTE: &str = "/api/work/{id}/edit";
pub(crate) const AGENTS_ROUTE: &str = "/api/agents";
pub(crate) const AGENT_PAUSE_ROUTE: &str = "/api/agents/{id}/pause";
pub(crate) const AGENT_RESUME_ROUTE: &str = "/api/agents/{id}/resume";
pub(crate) const CONTRACTS_ACTIVE_ROUTE: &str = "/api/contracts/active";
pub(crate) const CONTRACTS_COLLECTION_ROUTE: &str = "/api/contracts";
pub(crate) const CONTRACTS_ACTIVATE_ROUTE: &str = "/api/contracts/{id}/activate";
pub(crate) const ACTIVITY_ROUTE: &str = "/api/activity";
pub(crate) const RUN_DETAIL_ROUTE: &str = "/api/runs/{id}";
pub(crate) const EVENTS_ROUTE: &str = "/api/events";

pub(crate) const WORK_QUEUE_ROUTE: &str = "/api/work/{id}/queue";
pub(crate) const WORK_WAKE_ROUTE: &str = "/api/work/{id}/wake";
pub(crate) const WORK_REOPEN_ROUTE: &str = "/api/work/{id}/reopen";
pub(crate) const WORK_CANCEL_ROUTE: &str = "/api/work/{id}/cancel";
pub(crate) const WORK_OVERRIDE_ROUTE: &str = "/api/work/{id}/override";
pub(crate) const WORK_INTENTS_ROUTE: &str = "/api/work/{id}/intents";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RouteDto {
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) handler: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct WakeRequestDto {
    pub(crate) latest_reason: String,
    pub(crate) obligation_delta: Vec<String>,
}

pub(crate) fn board_state_routes() -> Vec<RouteDto> {
    vec![
        RouteDto {
            method: METHOD_POST,
            path: WORK_QUEUE_ROUTE,
            handler: "handle_submit_intent",
        },
        RouteDto {
            method: METHOD_POST,
            path: WORK_WAKE_ROUTE,
            handler: "handle_wake_work",
        },
        RouteDto {
            method: METHOD_POST,
            path: WORK_REOPEN_ROUTE,
            handler: "handle_submit_intent",
        },
        RouteDto {
            method: METHOD_POST,
            path: WORK_CANCEL_ROUTE,
            handler: "handle_submit_intent",
        },
        RouteDto {
            method: METHOD_POST,
            path: WORK_OVERRIDE_ROUTE,
            handler: "handle_submit_intent",
        },
    ]
}

pub(crate) fn work_state_routes() -> Vec<RouteDto> {
    vec![
        RouteDto {
            method: METHOD_POST,
            path: WORK_COLLECTION_ROUTE,
            handler: "handle_create_work",
        },
        RouteDto {
            method: METHOD_POST,
            path: WORK_UPDATE_ROUTE,
            handler: "handle_update_work",
        },
    ]
}

pub(crate) fn company_state_routes() -> Vec<RouteDto> {
    vec![RouteDto {
        method: METHOD_POST,
        path: COMPANIES_ROUTE,
        handler: "handle_create_company",
    }]
}

pub(crate) fn agent_state_routes() -> Vec<RouteDto> {
    vec![
        RouteDto {
            method: METHOD_POST,
            path: AGENTS_ROUTE,
            handler: "handle_create_agent",
        },
        RouteDto {
            method: METHOD_POST,
            path: AGENT_PAUSE_ROUTE,
            handler: "handle_set_agent_status",
        },
        RouteDto {
            method: METHOD_POST,
            path: AGENT_RESUME_ROUTE,
            handler: "handle_set_agent_status",
        },
    ]
}

pub(crate) fn contract_state_routes() -> Vec<RouteDto> {
    vec![
        RouteDto {
            method: METHOD_POST,
            path: CONTRACTS_COLLECTION_ROUTE,
            handler: "handle_create_contract_draft",
        },
        RouteDto {
            method: METHOD_POST,
            path: CONTRACTS_ACTIVATE_ROUTE,
            handler: "handle_activate_contract",
        },
    ]
}

pub(crate) fn query_routes() -> Vec<RouteDto> {
    vec![
        RouteDto {
            method: METHOD_GET,
            path: BOARD_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: COMPANIES_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: WORK_COLLECTION_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: WORK_DETAIL_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: AGENTS_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: CONTRACTS_ACTIVE_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: ACTIVITY_ROUTE,
            handler: "HttpTransport::handle",
        },
        RouteDto {
            method: METHOD_GET,
            path: RUN_DETAIL_ROUTE,
            handler: "HttpTransport::handle_get_run",
        },
    ]
}

pub(crate) fn runtime_routes() -> Vec<RouteDto> {
    vec![RouteDto {
        method: METHOD_POST,
        path: WORK_INTENTS_ROUTE,
        handler: "handle_submit_intent",
    }]
}
