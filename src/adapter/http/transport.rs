use serde::{Deserialize, Serialize};

use crate::{
    adapter::{
        memory::store::MemoryStore,
        sse::{after_commit_sse_stream, emit_after_commit, EmittedSseEvent},
        workspace::SystemWorkspace,
    },
    app::cmd::{
        activate_contract::{handle_activate_contract, ActivateContractCmd},
        create_agent::{handle_create_agent, CreateAgentCmd},
        create_company::{handle_create_company, CreateCompanyCmd},
        create_contract_draft::{handle_create_contract_draft, CreateContractDraftCmd},
        create_work::{handle_create_work, CreateWorkCmd},
        set_agent_status::{handle_set_agent_status, SetAgentStatusCmd},
        submit_intent::{handle_submit_intent, SubmitIntentCmd},
        update_work::{handle_update_work, UpdateWorkCmd},
        wake_work::{handle_wake_work, WakeWorkCmd},
    },
    model::{
        ActorId, ActorKind, AgentId, AgentStatus, CompanyId, ContractSetId, RunId,
        TransitionIntent, TransitionKind, TransitionRule, WorkId, WorkKind,
    },
    port::store::{
        ActivityReadModel, AgentReadModel, BoardReadModel, CommandStorePort, CompanyReadModel,
        ContractsReadModel, QueryStorePort, RunReadModel, StoreError, StoreErrorKind,
        WorkReadModel,
    },
};

use super::dto::{
    ACTIVITY_ROUTE, AGENTS_ROUTE, AGENT_PAUSE_ROUTE, AGENT_RESUME_ROUTE, BOARD_ROUTE,
    COMPANIES_ROUTE, CONTRACTS_ACTIVATE_ROUTE, CONTRACTS_ACTIVE_ROUTE, CONTRACTS_COLLECTION_ROUTE,
    EVENTS_ROUTE, METHOD_GET, METHOD_POST, RUN_DETAIL_ROUTE, WORK_CANCEL_ROUTE,
    WORK_COLLECTION_ROUTE, WORK_DETAIL_ROUTE, WORK_INTENTS_ROUTE, WORK_OVERRIDE_ROUTE,
    WORK_QUEUE_ROUTE, WORK_REOPEN_ROUTE, WORK_UPDATE_ROUTE, WORK_WAKE_ROUTE,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpRequest {
    pub(crate) method: &'static str,
    pub(crate) path: String,
    pub(crate) body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
    pub(crate) emitted_event: Option<EmittedSseEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct WakeWorkBody {
    latest_reason: String,
    obligation_delta: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CreateAgentBody {
    company_id: String,
    name: String,
    role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CreateCompanyBody {
    name: String,
    description: String,
    runtime_hard_stop_cents: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CreateContractBody {
    company_id: String,
    name: String,
    rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ActivateContractBody {
    company_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CreateWorkBody {
    company_id: String,
    parent_id: Option<String>,
    kind: WorkKind,
    title: String,
    body: String,
    contract_set_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct UpdateWorkBody {
    parent_id: Option<String>,
    title: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BoardResponse {
    sections: Vec<&'static str>,
    data: BoardReadModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct WorkResponse {
    routes: [&'static str; 2],
    data: WorkReadModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AgentResponse {
    route: &'static str,
    data: AgentReadModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CompanyResponse {
    route: &'static str,
    data: CompanyReadModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActivityResponse {
    route: &'static str,
    data: ActivityReadModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RunResponse {
    route: &'static str,
    data: RunReadModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContractsResponse {
    route: &'static str,
    data: ContractsReadModel,
}

const ERROR_ROUTE_NOT_FOUND: &str = "route not found";
const ERROR_REQUEST_BODY_REQUIRED: &str = "request body is required";
const ERROR_PATH_WORK_ID_MISMATCH: &str = "path work_id must match intent.work_id";
const BOARD_SECTIONS: &[&str] = &[
    "running_agents",
    "running_runs",
    "pending_wakes",
    "pending_wake_details",
    "blocked_work",
    "recent_transition_records",
    "recent_transition_details",
    "recent_gate_failures",
    "recent_gate_failure_details",
    "consumption_summary",
];
const WORK_ROUTES: [&str; 2] = [WORK_COLLECTION_ROUTE, WORK_DETAIL_ROUTE];

pub(crate) struct HttpTransport<S> {
    store: S,
    workspace: SystemWorkspace,
}

impl<S> HttpTransport<S>
where
    S: CommandStorePort + QueryStorePort,
{
    pub(crate) fn new(store: S) -> Self {
        Self {
            store,
            workspace: SystemWorkspace,
        }
    }

    pub(crate) fn handle(&self, req: HttpRequest) -> HttpResponse {
        match (req.method, req.path.as_str()) {
            (METHOD_GET, BOARD_ROUTE) => ok_json(&BoardResponse {
                sections: BOARD_SECTIONS.to_vec(),
                data: self.store.read_board(),
            }),
            (METHOD_GET, COMPANIES_ROUTE) => ok_json(&CompanyResponse {
                route: COMPANIES_ROUTE,
                data: self.store.read_companies(),
            }),
            (METHOD_GET, WORK_COLLECTION_ROUTE) => match self.store.read_work(None) {
                Ok(data) => ok_json(&WorkResponse {
                    routes: WORK_ROUTES,
                    data,
                }),
                Err(error) => store_error_json(error),
            },
            (METHOD_GET, AGENTS_ROUTE) => ok_json(&AgentResponse {
                route: AGENTS_ROUTE,
                data: self.store.read_agents(),
            }),
            (METHOD_GET, CONTRACTS_ACTIVE_ROUTE) => ok_json(&ContractsResponse {
                route: CONTRACTS_ACTIVE_ROUTE,
                data: self.store.read_contracts(),
            }),
            (METHOD_GET, ACTIVITY_ROUTE) => ok_json(&ActivityResponse {
                route: ACTIVITY_ROUTE,
                data: self.store.read_activity(),
            }),
            (METHOD_GET, _) if path_run_id(&req.path).is_some() => self.handle_get_run(req),
            (METHOD_POST, COMPANIES_ROUTE) => self.handle_create_company(req),
            (METHOD_POST, CONTRACTS_COLLECTION_ROUTE) => self.handle_create_contract(req),
            (METHOD_POST, WORK_COLLECTION_ROUTE) => self.handle_create_work(req),
            (METHOD_POST, AGENTS_ROUTE) => self.handle_create_agent(req),
            (METHOD_POST, _) if path_contract_revision(&req.path).is_some() => {
                self.handle_activate_contract(req)
            }
            (METHOD_POST, _) if path_agent_id(&req.path, AGENT_PAUSE_ROUTE).is_some() => {
                self.handle_set_agent_status(req, AgentStatus::Paused)
            }
            (METHOD_POST, _) if path_agent_id(&req.path, AGENT_RESUME_ROUTE).is_some() => {
                self.handle_set_agent_status(req, AgentStatus::Active)
            }
            (METHOD_POST, _) if path_work_id(&req.path, WORK_UPDATE_ROUTE).is_some() => {
                self.handle_update_work(req)
            }
            (METHOD_GET, EVENTS_ROUTE) => ok_json(&after_commit_sse_stream()),
            (METHOD_POST, _) if path_work_id(&req.path, WORK_INTENTS_ROUTE).is_some() => {
                self.handle_submit(req, SubmitRoute::RuntimeIntent)
            }
            (METHOD_POST, _) if path_work_id(&req.path, WORK_QUEUE_ROUTE).is_some() => {
                self.handle_submit(req, SubmitRoute::Queue)
            }
            (METHOD_POST, _) if path_work_id(&req.path, WORK_REOPEN_ROUTE).is_some() => {
                self.handle_submit(req, SubmitRoute::Reopen)
            }
            (METHOD_POST, _) if path_work_id(&req.path, WORK_CANCEL_ROUTE).is_some() => {
                self.handle_submit(req, SubmitRoute::Cancel)
            }
            (METHOD_POST, _) if path_work_id(&req.path, WORK_OVERRIDE_ROUTE).is_some() => {
                self.handle_submit(req, SubmitRoute::Override)
            }
            (METHOD_POST, _) if path_work_id(&req.path, WORK_WAKE_ROUTE).is_some() => {
                self.handle_wake(req)
            }
            (METHOD_GET, _) => match work_detail_id(&req.path) {
                Some(work_id) => match self.store.read_work(Some(&WorkId::from(work_id))) {
                    Ok(data) => ok_json(&WorkResponse {
                        routes: WORK_ROUTES,
                        data,
                    }),
                    Err(error) => store_error_json(error),
                },
                None => not_found_json(ERROR_ROUTE_NOT_FOUND),
            },
            _ => not_found_json(ERROR_ROUTE_NOT_FOUND),
        }
    }

    fn handle_submit(&self, req: HttpRequest, route: SubmitRoute) -> HttpResponse {
        let Some(work_id) = route.work_id(&req.path) else {
            return not_found_json(ERROR_ROUTE_NOT_FOUND);
        };
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let intent = match serde_json::from_str::<TransitionIntent>(body) {
            Ok(intent) => intent,
            Err(error) => {
                return bad_request_json(&format!("invalid TransitionIntent body: {error}"));
            }
        };

        if intent.work_id.as_str() != work_id {
            return bad_request_json(ERROR_PATH_WORK_ID_MISMATCH);
        }

        if let Err(message) = route.validate_kind(intent.kind) {
            return bad_request_json(message);
        }

        match handle_submit_intent(&self.store, &self.workspace, SubmitIntentCmd { intent }) {
            Ok(ack) => accepted_json(&ack, emit_after_commit(ack.after_commit_event_data.clone())),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_wake(&self, req: HttpRequest) -> HttpResponse {
        let Some(work_id) = path_work_id(&req.path, WORK_WAKE_ROUTE) else {
            return not_found_json(ERROR_ROUTE_NOT_FOUND);
        };
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<WakeWorkBody>(body) {
            Ok(body) => body,
            Err(error) => return bad_request_json(&format!("invalid wake body: {error}")),
        };

        match handle_wake_work(
            &self.store,
            WakeWorkCmd {
                work_id: WorkId::from(work_id),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("board"),
                source: "manual".to_owned(),
                latest_reason: body.latest_reason,
                obligation_delta: body.obligation_delta,
            },
        ) {
            Ok(ack) => accepted_json(
                &ack,
                emit_after_commit(format!("wake merged {}", ack.merged_count)),
            ),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_create_agent(&self, req: HttpRequest) -> HttpResponse {
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<CreateAgentBody>(body) {
            Ok(body) => body,
            Err(error) => return bad_request_json(&format!("invalid create agent body: {error}")),
        };

        match handle_create_agent(
            &self.store,
            CreateAgentCmd {
                company_id: CompanyId::from(body.company_id),
                name: body.name,
                role: body.role,
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_create_company(&self, req: HttpRequest) -> HttpResponse {
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<CreateCompanyBody>(body) {
            Ok(body) => body,
            Err(error) => {
                return bad_request_json(&format!("invalid create company body: {error}"))
            }
        };

        match handle_create_company(
            &self.store,
            CreateCompanyCmd {
                name: body.name,
                description: body.description,
                runtime_hard_stop_cents: body.runtime_hard_stop_cents,
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_create_contract(&self, req: HttpRequest) -> HttpResponse {
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<CreateContractBody>(body) {
            Ok(body) => body,
            Err(error) => {
                return bad_request_json(&format!("invalid create contract body: {error}"))
            }
        };

        match handle_create_contract_draft(
            &self.store,
            CreateContractDraftCmd {
                company_id: CompanyId::from(body.company_id),
                name: body.name,
                rules: body.rules,
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_create_work(&self, req: HttpRequest) -> HttpResponse {
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<CreateWorkBody>(body) {
            Ok(body) => body,
            Err(error) => return bad_request_json(&format!("invalid create work body: {error}")),
        };

        match handle_create_work(
            &self.store,
            CreateWorkCmd {
                company_id: CompanyId::from(body.company_id),
                parent_id: body.parent_id.as_deref().map(WorkId::from),
                kind: body.kind,
                title: body.title,
                body: body.body,
                contract_set_id: ContractSetId::from(body.contract_set_id),
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_set_agent_status(&self, req: HttpRequest, status: AgentStatus) -> HttpResponse {
        let Some(agent_id) = path_agent_id(
            &req.path,
            if status == AgentStatus::Paused {
                AGENT_PAUSE_ROUTE
            } else {
                AGENT_RESUME_ROUTE
            },
        ) else {
            return not_found_json(ERROR_ROUTE_NOT_FOUND);
        };

        match handle_set_agent_status(
            &self.store,
            SetAgentStatusCmd {
                agent_id: AgentId::from(agent_id),
                status,
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_update_work(&self, req: HttpRequest) -> HttpResponse {
        let Some(work_id) = path_work_id(&req.path, WORK_UPDATE_ROUTE) else {
            return not_found_json(ERROR_ROUTE_NOT_FOUND);
        };
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<UpdateWorkBody>(body) {
            Ok(body) => body,
            Err(error) => return bad_request_json(&format!("invalid update work body: {error}")),
        };

        match handle_update_work(
            &self.store,
            UpdateWorkCmd {
                work_id: WorkId::from(work_id),
                parent_id: body.parent_id.as_deref().map(WorkId::from),
                title: body.title,
                body: body.body,
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_activate_contract(&self, req: HttpRequest) -> HttpResponse {
        let Some(revision) = path_contract_revision(&req.path) else {
            return not_found_json(ERROR_ROUTE_NOT_FOUND);
        };
        let Some(body) = req.body.as_deref() else {
            return bad_request_json(ERROR_REQUEST_BODY_REQUIRED);
        };

        let body = match serde_json::from_str::<ActivateContractBody>(body) {
            Ok(body) => body,
            Err(error) => {
                return bad_request_json(&format!("invalid activate contract body: {error}"));
            }
        };

        match handle_activate_contract(
            &self.store,
            ActivateContractCmd {
                company_id: CompanyId::from(body.company_id),
                revision,
            },
        ) {
            Ok(ack) => ok_json(&ack),
            Err(error) => store_error_json(error),
        }
    }

    fn handle_get_run(&self, req: HttpRequest) -> HttpResponse {
        let Some(run_id) = path_run_id(&req.path) else {
            return not_found_json(ERROR_ROUTE_NOT_FOUND);
        };

        match self.store.read_run(&RunId::from(run_id)) {
            Ok(data) => ok_json(&RunResponse {
                route: RUN_DETAIL_ROUTE,
                data,
            }),
            Err(error) => store_error_json(error),
        }
    }
}

impl HttpTransport<MemoryStore> {
    pub(crate) fn demo() -> Self {
        Self::new(MemoryStore::demo())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmitRoute {
    RuntimeIntent,
    Queue,
    Reopen,
    Cancel,
    Override,
}

impl SubmitRoute {
    fn work_id(self, path: &str) -> Option<&str> {
        let route = match self {
            Self::RuntimeIntent => WORK_INTENTS_ROUTE,
            Self::Queue => WORK_QUEUE_ROUTE,
            Self::Reopen => WORK_REOPEN_ROUTE,
            Self::Cancel => WORK_CANCEL_ROUTE,
            Self::Override => WORK_OVERRIDE_ROUTE,
        };
        path_work_id(path, route)
    }

    fn validate_kind(self, kind: TransitionKind) -> Result<(), &'static str> {
        match self {
            Self::RuntimeIntent if kind.is_runtime_intent() => Ok(()),
            Self::Queue if kind == TransitionKind::Queue => Ok(()),
            Self::Reopen if kind == TransitionKind::Reopen => Ok(()),
            Self::Cancel if kind == TransitionKind::Cancel => Ok(()),
            Self::Override if kind == TransitionKind::OverrideComplete => Ok(()),
            Self::RuntimeIntent => Err("runtime intent route accepts runtime kinds only"),
            Self::Queue => Err("queue route requires kind=queue"),
            Self::Reopen => Err("reopen route requires kind=reopen"),
            Self::Cancel => Err("cancel route requires kind=cancel"),
            Self::Override => Err("override route requires kind=override_complete"),
        }
    }
}

fn path_work_id<'a>(path: &'a str, route_pattern: &str) -> Option<&'a str> {
    let (prefix, suffix) = route_pattern.split_once("{id}")?;
    path.strip_prefix(prefix)?
        .strip_suffix(suffix)
        .filter(|value| !value.is_empty())
}

fn path_agent_id<'a>(path: &'a str, route_pattern: &str) -> Option<&'a str> {
    let (prefix, suffix) = route_pattern.split_once("{id}")?;
    path.strip_prefix(prefix)?
        .strip_suffix(suffix)
        .filter(|value| !value.is_empty())
}

fn path_contract_revision(path: &str) -> Option<u32> {
    path_work_id(path, CONTRACTS_ACTIVATE_ROUTE)?
        .parse::<u32>()
        .ok()
}

fn path_run_id(path: &str) -> Option<&str> {
    path_work_id(path, RUN_DETAIL_ROUTE)
}

fn work_detail_id(path: &str) -> Option<&str> {
    path_work_id(path, WORK_DETAIL_ROUTE)
}

fn ok_json<T: Serialize>(body: &T) -> HttpResponse {
    HttpResponse {
        status: 200,
        body: serde_json::to_string(body).expect("json serialization should succeed"),
        emitted_event: None,
    }
}

fn accepted_json<T: Serialize>(body: &T, event: EmittedSseEvent) -> HttpResponse {
    HttpResponse {
        status: 202,
        body: serde_json::to_string(body).expect("json serialization should succeed"),
        emitted_event: Some(event),
    }
}

fn bad_request_json(message: &str) -> HttpResponse {
    error_json(400, message)
}

fn not_found_json(message: &str) -> HttpResponse {
    error_json(404, message)
}

fn store_error_json(error: StoreError) -> HttpResponse {
    match error.kind {
        StoreErrorKind::Conflict => error_json(409, &error.message),
        StoreErrorKind::NotFound => error_json(404, &error.message),
        StoreErrorKind::Unavailable => error_json(503, &error.message),
    }
}

fn error_json(status: u16, message: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: serde_json::json!({ "error": message }).to_string(),
        emitted_event: None,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crate::{
        adapter::http::dto::{
            ACTIVITY_ROUTE, AGENTS_ROUTE, BOARD_ROUTE, COMPANIES_ROUTE, CONTRACTS_ACTIVE_ROUTE,
            CONTRACTS_COLLECTION_ROUTE, EVENTS_ROUTE, RUN_DETAIL_ROUTE, WORK_COLLECTION_ROUTE,
            WORK_UPDATE_ROUTE,
        },
        adapter::memory::store::{
            MemoryStore, DEMO_AGENT_ID, DEMO_COMPANY_ID, DEMO_CONTRACT_SET_ID, DEMO_DOING_WORK_ID,
            DEMO_LEASE_ID, DEMO_TODO_WORK_ID,
        },
        app::cmd::activate_contract::{handle_activate_contract, ActivateContractCmd},
        app::cmd::append_comment::{handle_append_comment, AppendCommentCmd},
        app::cmd::claim_work::{handle_claim_work, ClaimWorkCmd},
        app::cmd::create_contract_draft::{handle_create_contract_draft, CreateContractDraftCmd},
        app::cmd::wake_work::{handle_wake_work, WakeWorkCmd},
        model::{
            workspace_fingerprint, ActorId, ActorKind, BillingKind, CompanyId, ConsumptionUsage,
            ProofHint, ProofHintKind, RuntimeKind, SessionId, TaskSession, TransitionIntent,
            TransitionKind, WorkId, WorkPatch,
        },
        port::store::{RecordConsumptionReq, StorePort},
    };

    use super::{HttpRequest, HttpTransport, METHOD_GET, METHOD_POST};

    #[test]
    fn write_routes_delegate_to_app_commands_without_kernel_logic() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = fs::read_to_string(repo_root.join("src/adapter/http/transport.rs"))
            .expect("transport source should load");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("transport source should contain production section");

        for required in [
            "handle_submit_intent(&self.store, &self.workspace, SubmitIntentCmd",
            "handle_wake_work(",
            "handle_create_company(",
            "handle_create_contract_draft(",
            "handle_create_work(",
            "handle_create_agent(",
            "handle_set_agent_status(",
            "handle_update_work(",
        ] {
            assert!(
                production_source.contains(required),
                "write transport should delegate through app command {required}",
            );
        }

        for forbidden in [
            "kernel::decide_transition",
            ".commit_decision(",
            ".merge_wake(",
            ".observe_changed_files(",
            ".run_gate_command(",
            ".record_consumption(",
            ".claim_lease(",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "write transport should not contain business-rule token {forbidden}",
            );
        }
    }

    #[test]
    fn query_routes_return_live_json_payloads() {
        let transport = HttpTransport::demo();
        let run_detail_path = RUN_DETAIL_ROUTE.replace("{id}", "run-1");

        for path in [
            BOARD_ROUTE.to_owned(),
            COMPANIES_ROUTE.to_owned(),
            WORK_COLLECTION_ROUTE.to_owned(),
            "/api/work/00000000-0000-4000-8000-000000000012".to_owned(),
            AGENTS_ROUTE.to_owned(),
            CONTRACTS_ACTIVE_ROUTE.to_owned(),
            ACTIVITY_ROUTE.to_owned(),
            run_detail_path.clone(),
            EVENTS_ROUTE.to_owned(),
        ] {
            let response = transport.handle(HttpRequest {
                method: METHOD_GET,
                path: path.clone(),
                body: None,
            });

            assert_eq!(response.status, 200, "path {path} should be live");
            assert!(!response.body.is_empty());
        }
    }

    fn work_detail_query_includes_persisted_comments() {
        let store = MemoryStore::demo();
        handle_append_comment(
            &store,
            AppendCommentCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                author_kind: ActorKind::Board,
                author_id: ActorId::from("00000000-0000-4000-8000-000000000032"),
                body: "needs follow-up".to_owned(),
            },
        )
        .expect("comment append should succeed");
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: format!("/api/work/{DEMO_DOING_WORK_ID}"),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("needs follow-up"));
        assert!(response.body.contains("comments"));
        assert!(response.body.contains("\"audit_entries\""));
    }

    #[test]
    fn create_and_update_work_routes_persist_tree_metadata() {
        let store = MemoryStore::demo();
        let transport = HttpTransport::new(store);

        let created = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: WORK_COLLECTION_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "company_id": DEMO_COMPANY_ID,
                    "parent_id": DEMO_TODO_WORK_ID,
                    "kind": "decision",
                    "title": "Decide release",
                    "body": "Compare rollout options",
                    "contract_set_id": DEMO_CONTRACT_SET_ID
                })
                .to_string(),
            ),
        });
        let created_work_id = serde_json::from_str::<serde_json::Value>(&created.body)
            .expect("create work response should parse")["work_id"]
            .as_str()
            .expect("create work response should include work_id")
            .to_owned();
        let updated = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: WORK_UPDATE_ROUTE.replace("{id}", &created_work_id),
            body: Some(
                serde_json::json!({
                    "parent_id": null,
                    "title": "Decide release now",
                    "body": "final draft"
                })
                .to_string(),
            ),
        });
        let detail = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: format!("/api/work/{created_work_id}"),
            body: None,
        });

        assert_eq!(created.status, 200);
        assert!(created.body.contains("\"status\":\"backlog\""));
        assert_eq!(updated.status, 200);
        assert!(detail.body.contains("\"kind\":\"decision\""));
        assert!(detail.body.contains("\"title\":\"Decide release now\""));
        assert!(detail.body.contains("\"body\":\"final draft\""));
        assert!(detail.body.contains("\"rev\":1"));
        assert!(detail.body.contains("\"contract_set_id\""));
        assert!(detail.body.contains("\"contract_rev\":1"));
        assert!(detail
            .body
            .contains("\"contract_name\":\"axiomnexus-rust-default\""));
        assert!(detail.body.contains("\"contract_status\":\"active\""));
    }

    #[test]
    fn work_detail_query_includes_work_scoped_audit_entries() {
        let store = MemoryStore::demo();
        handle_claim_work(
            &store,
            ClaimWorkCmd {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            },
        )
        .expect("claim should create transition audit");
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: format!("/api/work/{DEMO_TODO_WORK_ID}"),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"audit_entries\""));
        assert!(response.body.contains("\"event_kind\":\"transition\""));
        assert!(response.body.contains("\"after_status\":\"doing\""));
        assert!(response.body.contains("\"outcome\":\"accepted\""));
        assert!(response
            .body
            .contains("\"evidence_summary\":\"Claim Accepted with next status Doing\""));
    }

    #[test]
    fn work_detail_query_caps_audit_entries_to_recent_20() {
        let store = MemoryStore::demo();
        for seq in 0..25 {
            handle_append_comment(
                &store,
                AppendCommentCmd {
                    company_id: CompanyId::from(DEMO_COMPANY_ID),
                    work_id: WorkId::from(DEMO_TODO_WORK_ID),
                    author_kind: ActorKind::Board,
                    author_id: ActorId::from("00000000-0000-4000-8000-000000000032"),
                    body: format!("todo-{seq}"),
                },
            )
            .expect("comment append should succeed");
        }
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: format!("/api/work/{DEMO_TODO_WORK_ID}"),
            body: None,
        });
        let body = serde_json::from_str::<serde_json::Value>(&response.body)
            .expect("work detail response should parse");
        let audit_entries = body["data"]["items"][0]["audit_entries"]
            .as_array()
            .expect("work detail should expose audit entries");

        assert_eq!(response.status, 200);
        assert_eq!(audit_entries.len(), 20);
        assert!(audit_entries
            .iter()
            .any(|entry| entry["summary"] == serde_json::Value::String("todo-24".to_owned())));
        assert!(!audit_entries
            .iter()
            .any(|entry| entry["summary"] == serde_json::Value::String("todo-0".to_owned())));
    }

    #[test]
    fn activity_route_returns_audit_feed_fields() {
        let store = MemoryStore::demo();
        handle_claim_work(
            &store,
            ClaimWorkCmd {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
            },
        )
        .expect("claim should create transition audit");
        handle_wake_work(
            &store,
            WakeWorkCmd {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("00000000-0000-4000-8000-000000000034"),
                source: "manual".to_owned(),
                latest_reason: "gate retry".to_owned(),
                obligation_delta: vec!["cargo test".to_owned()],
            },
        )
        .expect("wake should create run audit");
        handle_append_comment(
            &store,
            AppendCommentCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                author_kind: ActorKind::Board,
                author_id: ActorId::from("00000000-0000-4000-8000-000000000033"),
                body: "audit note".to_owned(),
            },
        )
        .expect("comment append should succeed");
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: ACTIVITY_ROUTE.to_owned(),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"entries\""));
        assert!(response.body.contains("\"event_kind\""));
        assert!(response.body.contains("\"before_status\":\"todo\""));
        assert!(response.body.contains("\"after_status\":\"doing\""));
        assert!(response.body.contains("\"outcome\":\"accepted\""));
        assert!(response.body.contains("\"evidence_summary\""));
        assert!(response.body.contains("\"event_kind\":\"run\""));
        assert!(response.body.contains("\"actor_kind\":\"board\""));
        assert!(response.body.contains("\"source\":\"manual\""));
        assert!(response.body.contains("\"audit note\""));
    }

    #[test]
    fn run_detail_route_returns_run_and_current_session() {
        let store = MemoryStore::demo();
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-running"),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_DOING_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-running".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 2,
                last_record_id: None,
                last_decision_summary: Some("running session".to_owned()),
                last_gate_summary: None,
                updated_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(30),
            })
            .expect("session should persist");
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: RUN_DETAIL_ROUTE.replace("{id}", "run-1"),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"route\":\"/api/runs/{id}\""));
        assert!(response.body.contains("\"run_id\":\"run-1\""));
        assert!(response.body.contains("\"status\":\"running\""));
        assert!(response.body.contains("\"current_session\""));
        assert!(response
            .body
            .contains("\"runtime_session_id\":\"runtime-running\""));
    }

    #[test]
    fn board_and_agents_routes_include_consumption_rollups() {
        let store = MemoryStore::demo();
        store
            .record_consumption(RecordConsumptionReq {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                run_id: crate::model::RunId::from("run-1"),
                billing_kind: BillingKind::Api,
                usage: ConsumptionUsage {
                    input_tokens: 120,
                    output_tokens: 48,
                    run_seconds: 3,
                    estimated_cost_cents: Some(7),
                },
            })
            .expect("consumption event should persist");
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-queued"),
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-queued".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 2,
                last_record_id: None,
                last_decision_summary: Some("queued session".to_owned()),
                last_gate_summary: None,
                updated_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(20),
            })
            .expect("session should persist");
        handle_wake_work(
            &store,
            WakeWorkCmd {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("board"),
                source: "manual".to_owned(),
                latest_reason: "scheduler queue".to_owned(),
                obligation_delta: vec!["follow up".to_owned()],
            },
        )
        .expect("wake should create queued run");
        let transport = HttpTransport::new(store);

        let board = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: BOARD_ROUTE.to_owned(),
            body: None,
        });
        let agents = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: AGENTS_ROUTE.to_owned(),
            body: None,
        });

        assert_eq!(board.status, 200);
        assert!(board.body.contains("\"consumption_summary\""));
        assert!(board.body.contains("\"total_turns\":1"));
        assert!(board.body.contains("\"total_estimated_cost_cents\":7"));
        assert!(board.body.contains("\"running_runs\""));
        assert!(board.body.contains("\"run_id\":\"run-1\""));
        assert!(board
            .body
            .contains("\"lease_id\":\"00000000-0000-4000-8000-000000000013\""));
        assert!(board.body.contains("\"pending_wake_details\""));
        assert!(board.body.contains("\"latest_reason\":\"scheduler queue\""));
        assert!(board.body.contains("\"obligations\":[\"follow up\"]"));
        assert!(board.body.contains("\"recent_transition_details\""));
        assert!(board.body.contains("\"recent_gate_failure_details\""));
        assert_eq!(agents.status, 200);
        assert!(agents.body.contains("\"consumption_by_agent\""));
        assert!(agents.body.contains(DEMO_AGENT_ID));
        assert!(agents.body.contains("\"total_input_tokens\":120"));
        assert!(agents.body.contains("\"registered_agents\""));
        assert!(agents.body.contains("\"recent_runs\""));
        assert!(agents.body.contains("\"current_sessions\""));
        assert!(agents.body.contains("\"run_id\":\"run-2\""));
        assert!(agents
            .body
            .contains("\"runtime_session_id\":\"runtime-queued\""));
        assert!(agents.body.contains("\"status\":\"active\""));
    }

    #[test]
    fn create_agent_route_registers_agent_profile() {
        let store = MemoryStore::demo();
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: AGENTS_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "company_id": DEMO_COMPANY_ID,
                    "name": "Release Operator",
                    "role": "release_manager"
                })
                .to_string(),
            ),
        });
        let agents = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: AGENTS_ROUTE.to_owned(),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"status\":\"active\""));
        assert!(agents.body.contains("Release Operator"));
        assert!(agents.body.contains("release_manager"));
    }

    #[test]
    fn companies_route_creates_and_reads_company_profile() {
        let transport = HttpTransport::demo();

        let created = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: COMPANIES_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "name": "Acme",
                    "description": "release scope"
                })
                .to_string(),
            ),
        });
        let companies = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: COMPANIES_ROUTE.to_owned(),
            body: None,
        });
        let created_company_id = serde_json::from_str::<serde_json::Value>(&created.body)
            .expect("create company response should be json")["company_id"]
            .as_str()
            .expect("company_id should be present")
            .to_owned();
        let companies_json =
            serde_json::from_str::<serde_json::Value>(&companies.body).expect("companies json");
        let created_company = companies_json["data"]["items"]
            .as_array()
            .expect("companies items should be an array")
            .iter()
            .find(|item| item["company_id"].as_str() == Some(created_company_id.as_str()))
            .expect("created company should be present");

        assert_eq!(created.status, 200);
        assert!(created.body.contains("company_id"));
        assert_eq!(created_company["name"].as_str(), Some("Acme"));
        assert_eq!(
            created_company["description"].as_str(),
            Some("release scope")
        );
        assert_eq!(created_company["agent_count"].as_u64(), Some(0));
        assert_eq!(created_company["work_count"].as_u64(), Some(0));
        assert!(created_company["active_contract_set_id"].is_null());
        assert!(created_company["active_contract_revision"].is_null());
    }

    #[test]
    fn pause_and_resume_agent_routes_update_agent_status() {
        let store = MemoryStore::demo();
        handle_wake_work(
            &store,
            WakeWorkCmd {
                work_id: WorkId::from(DEMO_TODO_WORK_ID),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("board"),
                source: "manual".to_owned(),
                latest_reason: "scheduler queue".to_owned(),
                obligation_delta: vec!["follow up".to_owned()],
            },
        )
        .expect("wake should create queued run");
        let transport = HttpTransport::new(store);

        let paused = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: format!("/api/agents/{DEMO_AGENT_ID}/pause"),
            body: None,
        });
        let agents_after_pause = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: AGENTS_ROUTE.to_owned(),
            body: None,
        });
        let resumed = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: format!("/api/agents/{DEMO_AGENT_ID}/resume"),
            body: None,
        });

        assert_eq!(paused.status, 200);
        assert!(paused.body.contains("\"status\":\"paused\""));
        assert!(agents_after_pause.body.contains("\"status\":\"paused\""));
        assert_eq!(resumed.status, 200);
        assert!(resumed.body.contains("\"status\":\"active\""));
    }

    #[test]
    fn resume_route_rejects_terminated_agent() {
        let transport = HttpTransport::demo();

        let response = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: "/api/agents/00000000-0000-4000-8000-000000000005/resume".to_owned(),
            body: None,
        });

        assert_eq!(response.status, 409);
        assert!(response.body.contains("terminated"));
    }

    #[test]
    fn contracts_route_returns_revision_list_and_rules_view() {
        let transport = HttpTransport::demo();

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: CONTRACTS_ACTIVE_ROUTE.to_owned(),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"revisions\""));
        assert!(response.body.contains("\"rules\""));
        assert!(response.body.contains("\"status\":\"active\""));
        assert!(response.body.contains("\"status\":\"draft\""));
        assert!(response.body.contains("\"status\":\"retired\""));
    }

    #[test]
    fn contracts_route_reflects_activated_revision() {
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
        handle_activate_contract(
            &store,
            ActivateContractCmd {
                company_id: CompanyId::from(DEMO_COMPANY_ID),
                revision: 3,
            },
        )
        .expect("activate should succeed");
        let transport = HttpTransport::new(store);

        let response = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: CONTRACTS_ACTIVE_ROUTE.to_owned(),
            body: None,
        });

        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"revision\":3"));
        assert!(response.body.contains("axiomnexus-rust-next"));
    }

    #[test]
    fn contract_write_routes_create_and_activate_revision() {
        let store = MemoryStore::demo();
        let rules = store.read_contracts().rules;
        let transport = HttpTransport::new(store);

        let created = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: CONTRACTS_COLLECTION_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "company_id": DEMO_COMPANY_ID,
                    "name": "axiomnexus-rust-next",
                    "rules": rules
                })
                .to_string(),
            ),
        });
        let activated = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: "/api/contracts/3/activate".to_owned(),
            body: Some(
                serde_json::json!({
                    "company_id": DEMO_COMPANY_ID
                })
                .to_string(),
            ),
        });

        assert_eq!(created.status, 200);
        assert!(created.body.contains("\"revision\":3"));
        assert_eq!(activated.status, 200);
        assert!(activated.body.contains("\"revision\":3"));
    }

    #[test]
    fn onboarding_route_flow_works_for_a_new_company() {
        let store = MemoryStore::demo();
        let rules = store.read_contracts().rules;
        let transport = HttpTransport::new(store);

        let created_company = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: COMPANIES_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "name": "Scenario Labs",
                    "description": "http onboarding"
                })
                .to_string(),
            ),
        });
        let company_id = serde_json::from_str::<serde_json::Value>(&created_company.body)
            .expect("create company response should be json")["company_id"]
            .as_str()
            .expect("company_id should be present")
            .to_owned();
        let created_contract = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: CONTRACTS_COLLECTION_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "company_id": company_id,
                    "name": "scenario-contract",
                    "rules": rules
                })
                .to_string(),
            ),
        });
        let revision = serde_json::from_str::<serde_json::Value>(&created_contract.body)
            .expect("create contract response should be json")["revision"]
            .as_u64()
            .expect("revision should be present");
        let activated = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: format!("/api/contracts/{revision}/activate"),
            body: Some(
                serde_json::json!({
                    "company_id": company_id
                })
                .to_string(),
            ),
        });
        let companies = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: COMPANIES_ROUTE.to_owned(),
            body: None,
        });
        let companies_json =
            serde_json::from_str::<serde_json::Value>(&companies.body).expect("companies json");
        let active_contract_set_id = companies_json["data"]["items"]
            .as_array()
            .expect("companies items should be an array")
            .iter()
            .find(|item| item["company_id"].as_str() == Some(company_id.as_str()))
            .and_then(|item| item["active_contract_set_id"].as_str())
            .expect("new company should expose active contract")
            .to_owned();
        let created_work = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: WORK_COLLECTION_ROUTE.to_owned(),
            body: Some(
                serde_json::json!({
                    "company_id": company_id,
                    "parent_id": null,
                    "kind": "task",
                    "title": "Scenario Task",
                    "body": "Exercise queue/wake",
                    "contract_set_id": active_contract_set_id
                })
                .to_string(),
            ),
        });
        let work_id = serde_json::from_str::<serde_json::Value>(&created_work.body)
            .expect("create work response should be json")["work_id"]
            .as_str()
            .expect("work_id should be present")
            .to_owned();
        let work_detail = transport.handle(HttpRequest {
            method: METHOD_GET,
            path: format!("/api/work/{work_id}"),
            body: None,
        });

        assert_eq!(created_company.status, 200);
        assert_eq!(created_contract.status, 200);
        assert_eq!(activated.status, 200);
        assert_eq!(created_work.status, 200);
        assert!(work_detail.body.contains("\"title\":\"Scenario Task\""));
        assert!(work_detail.body.contains("\"rev\":0"));
        assert!(work_detail
            .body
            .contains("\"contract_name\":\"scenario-contract\""));
        assert!(work_detail.body.contains("\"contract_rev\":1"));
    }

    #[test]
    fn runtime_intent_route_commits_and_emits_after_commit_sse() {
        let transport = HttpTransport::demo();
        let response = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: format!("/api/work/{DEMO_DOING_WORK_ID}/intents"),
            body: Some(
                serde_json::to_string(&TransitionIntent {
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                    lease_id: crate::model::LeaseId::from(DEMO_LEASE_ID),
                    expected_rev: 1,
                    kind: TransitionKind::Block,
                    patch: WorkPatch {
                        summary: "blocked".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: vec!["needs reviewer".to_owned()],
                    },
                    note: Some("blocked by review".to_owned()),
                    proof_hints: vec![ProofHint {
                        kind: ProofHintKind::Summary,
                        value: "blocked".to_owned(),
                    }],
                })
                .expect("intent json should serialize"),
            ),
        });

        assert_eq!(response.status, 202);
        assert!(response.body.contains("accepted"));
        assert!(response
            .emitted_event
            .as_ref()
            .expect("after commit event")
            .data
            .contains("\"event_kind\":\"transition\""));
        assert!(response
            .emitted_event
            .as_ref()
            .expect("after commit event")
            .data
            .contains("\"summary\":\"Block Accepted with next status Blocked\""));
        assert_eq!(
            response.emitted_event.expect("after commit event").emission,
            "after commit only"
        );
    }

    #[test]
    fn runtime_intent_route_does_not_emit_after_commit_event_on_failed_write() {
        let transport = HttpTransport::demo();
        let response = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: format!("/api/work/{DEMO_DOING_WORK_ID}/intents"),
            body: Some(
                serde_json::to_string(&TransitionIntent {
                    work_id: WorkId::from(DEMO_DOING_WORK_ID),
                    agent_id: crate::model::AgentId::from(DEMO_AGENT_ID),
                    lease_id: crate::model::LeaseId::from(DEMO_LEASE_ID),
                    expected_rev: 0,
                    kind: TransitionKind::ProposeProgress,
                    patch: WorkPatch {
                        summary: "stale turn".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    note: None,
                    proof_hints: Vec::new(),
                })
                .expect("intent json should serialize"),
            ),
        });

        assert_eq!(response.status, 409);
        assert!(response.body.contains("expected_rev"));
        assert!(response.emitted_event.is_none());
    }

    #[test]
    fn wake_route_merges_pending_wake_and_emits_event() {
        let transport = HttpTransport::demo();
        let response = transport.handle(HttpRequest {
            method: METHOD_POST,
            path: "/api/work/00000000-0000-4000-8000-000000000011/wake".to_owned(),
            body: Some(
                serde_json::json!({
                    "latest_reason": "gate failed",
                    "obligation_delta": ["cargo test"]
                })
                .to_string(),
            ),
        });

        assert_eq!(response.status, 202);
        assert!(response.body.contains("merge or create"));
        assert_eq!(
            response.emitted_event.expect("wake event").emission,
            "after commit only"
        );
    }
}
