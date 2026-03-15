mod commit;
mod query;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::local::{Db, SurrealKv},
    types::SurrealValue,
    Surreal,
};

use crate::{
    adapter::store_support,
    kernel,
    model::{
        ActorKind, AgentId, AgentStatus, BillingKind, CompanyId, CompanyProfile, ContractSet,
        ContractSetId, ContractSetStatus, DecisionOutcome, LeaseEffect, LeaseId,
        LeaseReleaseReason, PendingWake, Priority, RunId, RunStatus, RuntimeKind, SessionId,
        TaskSession, Timestamp, TransitionKind, TransitionRecord, WorkId, WorkKind, WorkLease,
        WorkSnapshot, WorkStatus,
    },
    port::store::{
        ActivateContractReq, ActivateContractRes, ActivityEntryView, ActivityReadModel,
        AgentConsumptionSummaryView, AgentFacts, AgentReadModel, AgentRunView,
        AgentSessionSummaryView, AgentSummaryView, AppendCommentReq, AppendCommentRes,
        BoardReadModel, ClaimLeaseReq, ClaimLeaseRes, CommitDecisionReq, CommitDecisionRes,
        CompanyReadModel, CompanySummaryView, ConsumptionSummaryView, ContractRevisionView,
        ContractsReadModel, CreateAgentReq, CreateAgentRes, CreateCompanyReq, CreateCompanyRes,
        CreateContractDraftReq, CreateContractDraftRes, CreateWorkReq, CreateWorkRes, MergeWakeReq,
        PendingWakeSummaryView, QueuedRunCandidate, ReapedRun, RecordConsumptionReq, RunReadModel,
        RunningRunView, RuntimeTurnContext, SessionKey, SetAgentStatusReq, SetAgentStatusRes,
        StoreError, StoreErrorKind, StorePort, UpdateWorkReq, UpdateWorkRes, WorkCommentView,
        WorkContext, WorkReadModel, WorkSummary,
    },
};

pub(crate) const DEFAULT_NAMESPACE: &str = "axiomnexus";
pub(crate) const DEFAULT_DATABASE: &str = "primary";
const STORE_META_RECORD: &str = "store_meta:primary";
const SNAPSHOT_FORMAT: &str = "axiomnexus.surreal-snapshot.v1";
const CORE_SCHEMA_SQL: &str = concat!(
    "DEFINE TABLE store_meta SCHEMALESS;\n",
    "DEFINE TABLE company SCHEMALESS;\n",
    "DEFINE TABLE agent SCHEMALESS;\n",
    "DEFINE TABLE contract_revision SCHEMALESS;\n",
    "DEFINE TABLE work SCHEMALESS;\n",
    "DEFINE TABLE lease SCHEMALESS;\n",
    "DEFINE TABLE pending_wake SCHEMALESS;\n",
    "DEFINE TABLE run SCHEMALESS;\n",
    "DEFINE TABLE task_session SCHEMALESS;\n",
    "DEFINE TABLE transition_record SCHEMALESS;\n",
    "DEFINE TABLE work_comment SCHEMALESS;\n",
    "DEFINE TABLE consumption_event SCHEMALESS;\n",
    "DEFINE TABLE activity_event SCHEMALESS;\n",
    "DEFINE INDEX IF NOT EXISTS agent_company_status_idx ON TABLE agent COLUMNS company_id, status;\n",
    "DEFINE INDEX IF NOT EXISTS contract_company_status_idx ON TABLE contract_revision COLUMNS company_id, status;\n",
    "DEFINE INDEX IF NOT EXISTS work_company_status_idx ON TABLE work COLUMNS company_id, status;\n",
    "DEFINE INDEX IF NOT EXISTS work_parent_idx ON TABLE work COLUMNS parent_id;\n",
    "DEFINE INDEX IF NOT EXISTS lease_work_release_idx ON TABLE lease COLUMNS work_id, released_at_secs;\n",
    "DEFINE INDEX IF NOT EXISTS lease_run_release_idx ON TABLE lease COLUMNS run_id, released_at_secs;\n",
    "DEFINE INDEX IF NOT EXISTS run_status_created_idx ON TABLE run COLUMNS status, created_at_secs;\n",
    "DEFINE INDEX IF NOT EXISTS run_work_status_idx ON TABLE run COLUMNS work_id, status;\n",
    "DEFINE INDEX IF NOT EXISTS transition_work_happened_idx ON TABLE transition_record COLUMNS work_id, happened_at_secs;\n",
    "DEFINE INDEX IF NOT EXISTS work_comment_work_created_idx ON TABLE work_comment COLUMNS work_id, created_at_secs;\n",
    "DEFINE INDEX IF NOT EXISTS activity_work_happened_idx ON TABLE activity_event COLUMNS work_id, happened_at_secs;\n",
    "DEFINE INDEX IF NOT EXISTS activity_happened_idx ON TABLE activity_event COLUMNS happened_at_secs;\n"
);

pub(crate) struct SurrealStore {
    commit_lock: Mutex<()>,
    runtime: tokio::runtime::Runtime,
    db: Surreal<Db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub(crate) struct StoreMeta {
    pub(crate) next_company_seq: u64,
    pub(crate) next_agent_seq: u64,
    pub(crate) next_contract_seq: u64,
    pub(crate) next_work_seq: u64,
    pub(crate) next_run_seq: u64,
    pub(crate) next_lease_seq: u64,
    pub(crate) next_comment_seq: u64,
    pub(crate) next_consumption_seq: u64,
    pub(crate) tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct CompanyDoc {
    company_id: String,
    name: String,
    description: String,
    runtime_hard_stop_cents: Option<u64>,
    #[serde(default)]
    recorded_estimated_cost_cents: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, SurrealValue)]
struct CompanyKeyDoc {
    company_id: String,
}

#[derive(Debug, Clone, Deserialize, SurrealValue)]
struct ActiveContractDoc {
    company_id: String,
    contract_set_id: String,
    revision: u32,
}

#[derive(Debug, Clone, Deserialize, SurrealValue)]
struct LeaseAgentDoc {
    agent_id: String,
    released_at_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, SurrealValue)]
struct LeaseRunDoc {
    lease_id: String,
    run_id: Option<String>,
    work_id: String,
    agent_id: String,
    released_at_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, SurrealValue)]
struct ConsumptionAgentDoc {
    agent_id: String,
    input_tokens: u64,
    output_tokens: u64,
    run_seconds: u64,
    estimated_cost_cents: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct AgentDoc {
    agent_id: String,
    company_id: String,
    name: String,
    role: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct ContractRevisionDoc {
    contract_set_id: String,
    company_id: String,
    revision: u32,
    name: String,
    status: String,
    rules_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct WorkDoc {
    work_id: String,
    company_id: String,
    parent_id: Option<String>,
    kind: String,
    title: String,
    body: String,
    status: String,
    priority: String,
    assignee_agent_id: Option<String>,
    active_lease_id: Option<String>,
    rev: u64,
    contract_set_id: String,
    contract_rev: u32,
    created_at_secs: u64,
    updated_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct LeaseDoc {
    lease_id: String,
    company_id: String,
    work_id: String,
    agent_id: String,
    run_id: Option<String>,
    acquired_at_secs: u64,
    expires_at_secs: Option<u64>,
    released_at_secs: Option<u64>,
    release_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct PendingWakeDoc {
    work_id: String,
    obligations: Vec<String>,
    count: u32,
    latest_reason: String,
    merged_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct RunDoc {
    run_id: String,
    company_id: String,
    agent_id: String,
    work_id: String,
    status: String,
    created_at_secs: u64,
    updated_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct SessionDoc {
    session_id: String,
    company_id: String,
    agent_id: String,
    work_id: String,
    runtime: String,
    runtime_session_id: String,
    cwd: String,
    workspace_fingerprint: String,
    contract_rev: u32,
    last_record_id: Option<String>,
    last_decision_summary: Option<String>,
    last_gate_summary: Option<String>,
    updated_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct ConsumptionEventDoc {
    event_id: String,
    company_id: String,
    agent_id: String,
    run_id: String,
    billing_kind: String,
    input_tokens: u64,
    output_tokens: u64,
    run_seconds: u64,
    estimated_cost_cents: Option<u64>,
    created_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct WorkCommentDoc {
    comment_id: String,
    company_id: String,
    work_id: String,
    author_kind: String,
    author_id: String,
    source: Option<String>,
    body: String,
    created_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct TransitionRecordDoc {
    record_id: String,
    company_id: String,
    work_id: String,
    actor_kind: String,
    actor_id: String,
    run_id: Option<String>,
    session_id: Option<String>,
    lease_id: Option<String>,
    expected_rev: u64,
    contract_set_id: String,
    contract_rev: u32,
    before_status: String,
    after_status: Option<String>,
    outcome: String,
    #[serde(default = "default_json_array")]
    reasons_json: serde_json::Value,
    kind: String,
    patch_summary: String,
    resolved_obligations: Vec<String>,
    declared_risks: Vec<String>,
    failed_gates: Vec<String>,
    gate_results_json: serde_json::Value,
    #[serde(default = "default_json_object")]
    evidence_json: serde_json::Value,
    evidence_summary: Option<String>,
    evidence_refs_json: serde_json::Value,
    happened_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
struct ActivityEventDoc {
    event_id: String,
    work_id: String,
    event_kind: String,
    summary: String,
    actor_kind: Option<String>,
    actor_id: Option<String>,
    source: Option<String>,
    before_status: Option<String>,
    after_status: Option<String>,
    outcome: Option<String>,
    evidence_summary: Option<String>,
    happened_at_secs: u64,
    priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoreSnapshotEnvelope {
    pub(crate) format: String,
    pub(crate) checksum_fnv64: String,
    pub(crate) exported_at_secs: u64,
    pub(crate) state: StoreSnapshotState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoreSnapshotState {
    store_meta: StoreMeta,
    companies: Vec<CompanyDoc>,
    agents: Vec<AgentDoc>,
    contracts: Vec<ContractRevisionDoc>,
    work_items: Vec<WorkDoc>,
    leases: Vec<LeaseDoc>,
    pending_wakes: Vec<PendingWakeDoc>,
    runs: Vec<RunDoc>,
    sessions: Vec<SessionDoc>,
    transition_records: Vec<TransitionRecordDoc>,
    comments: Vec<WorkCommentDoc>,
    consumption_events: Vec<ConsumptionEventDoc>,
    activity_events: Vec<ActivityEventDoc>,
}

impl StoreSnapshotState {
    fn normalize(&mut self) {
        self.companies
            .sort_by(|left, right| left.company_id.cmp(&right.company_id));
        self.agents
            .sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        self.contracts.sort_by(|left, right| {
            left.contract_set_id
                .cmp(&right.contract_set_id)
                .then_with(|| left.revision.cmp(&right.revision))
        });
        self.work_items
            .sort_by(|left, right| left.work_id.cmp(&right.work_id));
        self.leases
            .sort_by(|left, right| left.lease_id.cmp(&right.lease_id));
        self.pending_wakes
            .sort_by(|left, right| left.work_id.cmp(&right.work_id));
        self.runs
            .sort_by(|left, right| left.run_id.cmp(&right.run_id));
        self.sessions
            .sort_by(|left, right| left.session_id.cmp(&right.session_id));
        self.transition_records
            .sort_by(|left, right| left.record_id.cmp(&right.record_id));
        self.comments
            .sort_by(|left, right| left.comment_id.cmp(&right.comment_id));
        self.consumption_events
            .sort_by(|left, right| left.event_id.cmp(&right.event_id));
        self.activity_events
            .sort_by(|left, right| left.event_id.cmp(&right.event_id));
    }
}

#[derive(Debug, Clone)]
struct TimedActivityView {
    sort_key: u64,
    priority: u8,
    view: ActivityEntryView,
}

#[derive(Debug, Clone, Deserialize)]
struct DemoContractTemplate {
    name: String,
    revision: u32,
    rules: Vec<crate::model::TransitionRule>,
}

impl SurrealStore {
    pub(crate) fn migrate(store_url: &str) -> Result<(), StoreError> {
        let _store = Self::open(store_url)?;
        Ok(())
    }

    pub(crate) fn open(store_url: &str) -> Result<Self, StoreError> {
        let path = store_path(store_url)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| unavailable(&format!("surreal runtime init failed: {error}")))?;

        let db = runtime.block_on(async {
            let db = Surreal::new::<SurrealKv>(path.as_path())
                .await
                .map_err(|error| unavailable(&format!("surreal open failed: {error}")))?;
            db.use_ns(DEFAULT_NAMESPACE)
                .use_db(DEFAULT_DATABASE)
                .await
                .map_err(|error| {
                    unavailable(&format!("surreal ns/db selection failed: {error}"))
                })?;
            Ok::<_, StoreError>(db)
        })?;

        let store = Self {
            commit_lock: Mutex::new(()),
            runtime,
            db,
        };
        store.bootstrap()?;
        store.seed_demo_state()?;
        Ok(store)
    }

    pub(crate) fn export_snapshot_to_file(
        store_url: &str,
        export_path: &Path,
    ) -> Result<StoreSnapshotEnvelope, StoreError> {
        let store = Self::open(store_url)?;
        let export = store.export_snapshot_envelope()?;
        drop(store);
        write_snapshot_envelope(&export, export_path)?;
        Ok(export)
    }

    pub(crate) fn import_snapshot_from_file(
        store_url: &str,
        export_path: &Path,
    ) -> Result<StoreSnapshotEnvelope, StoreError> {
        let export = read_snapshot_envelope(export_path)?;
        let state = decode_snapshot_state(&export)?;
        let store = Self::open(store_url)?;
        store.replace_from_snapshot(&state)?;
        drop(store);
        Ok(export)
    }

    pub(crate) fn claim_lease(&self, req: ClaimLeaseReq) -> Result<ClaimLeaseRes, StoreError> {
        let acquired_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;
        let snapshot = self
            .select_record::<WorkDoc>(&work_record_id(req.work_id.as_str()))?
            .ok_or_else(|| not_found("claim_lease", req.work_id.as_str()))?;
        let prepared = prepare_claim_acquire(
            self,
            &snapshot,
            &req.agent_id,
            &req.lease_id,
            acquired_at_secs,
            "claim_lease",
        )?;
        self.upsert_record(&run_record_id(&prepared.run.run_id), prepared.run.clone())?;
        if let Some(run_activity) = prepared.run_activity {
            self.upsert_record(
                &activity_event_record_id(&run_activity.event_id),
                run_activity,
            )?;
        }
        self.upsert_record(
            &lease_record_id(prepared.lease.lease_id.as_str()),
            LeaseDoc::from_model(&prepared.lease),
        )?;
        let work_snapshot = snapshot.clone().into_snapshot()?;
        let record = crate::kernel::claim_transition_record(
            &work_snapshot,
            &prepared.lease,
            timestamp(acquired_at_secs),
        );
        self.upsert_record(
            &work_record_id(req.work_id.as_str()),
            WorkDoc {
                status: work_status_label(WorkStatus::Doing).to_owned(),
                assignee_agent_id: Some(req.agent_id.to_string()),
                active_lease_id: Some(req.lease_id.to_string()),
                rev: snapshot.rev + 1,
                updated_at_secs: acquired_at_secs,
                ..snapshot
            },
        )?;
        self.upsert_record(
            &transition_record_id(record.record_id.as_str()),
            TransitionRecordDoc::from_model(&record)?,
        )?;
        self.upsert_record(
            &activity_event_record_id(record.record_id.as_str()),
            ActivityEventDoc::from_transition_record(&record),
        )?;

        Ok(ClaimLeaseRes {
            lease: prepared.lease,
        })
    }

    pub(crate) fn load_store_meta(&self) -> Result<StoreMeta, StoreError> {
        self.runtime.block_on(async {
            let mut response = self
                .db
                .query(format!("SELECT * FROM {STORE_META_RECORD}"))
                .await
                .map_err(|error| unavailable(&format!("surreal store_meta load failed: {error}")))?
                .check()
                .map_err(|error| {
                    unavailable(&format!("surreal store_meta query failed: {error}"))
                })?;

            let meta: Option<StoreMeta> = response.take(0).map_err(|error| {
                unavailable(&format!("surreal store_meta decode failed: {error}"))
            })?;

            meta.ok_or_else(|| unavailable("surreal store_meta is missing after bootstrap"))
        })
    }

    fn bootstrap(&self) -> Result<(), StoreError> {
        self.runtime.block_on(async {
            let mut existing_response = self
                .db
                .query(format!("SELECT * FROM {STORE_META_RECORD}"))
                .await
                .map_err(|error| {
                    unavailable(&format!("surreal store_meta probe failed: {error}"))
                })?;
            let existing_meta: Result<Option<StoreMeta>, StoreError> =
                existing_response.take(0).map_err(|error| {
                    unavailable(&format!("surreal store_meta probe decode failed: {error}"))
                });

            if matches!(existing_meta, Ok(Some(_))) {
                return Ok(());
            }

            self.db
                .query(CORE_SCHEMA_SQL)
                .await
                .map_err(|error| unavailable(&format!("surreal schema bootstrap failed: {error}")))?
                .check()
                .map_err(|error| unavailable(&format!("surreal schema check failed: {error}")))?;

            if !matches!(existing_meta, Ok(Some(_))) {
                self.db
                    .query(format!("CREATE {STORE_META_RECORD} CONTENT $meta"))
                    .bind(("meta", StoreMeta::default()))
                    .await
                    .map_err(|error| {
                        unavailable(&format!("surreal store_meta create failed: {error}"))
                    })?
                    .check()
                    .map_err(|error| {
                        unavailable(&format!("surreal store_meta check failed: {error}"))
                    })?;
            }

            Ok(())
        })
    }

    fn seed_demo_state(&self) -> Result<(), StoreError> {
        if !self.select_table::<CompanyDoc>("company")?.is_empty() {
            return Ok(());
        }

        use crate::adapter::memory::store::{
            DEMO_AGENT_ID, DEMO_COMPANY_ID, DEMO_CONTRACT_SET_ID, DEMO_DOING_WORK_ID,
            DEMO_LEASE_ID, DEMO_TODO_WORK_ID,
        };

        let contract = serde_json::from_str::<DemoContractTemplate>(include_str!(
            "../../../samples/company-rust-contract.example.json"
        ))
        .map_err(|error| unavailable(&format!("surreal demo contract parse failed: {error}")))?;

        self.upsert_record(
            &company_record_id(DEMO_COMPANY_ID),
            CompanyDoc {
                company_id: DEMO_COMPANY_ID.to_owned(),
                name: "AxiomNexus Demo".to_owned(),
                description: "demo company".to_owned(),
                runtime_hard_stop_cents: None,
                recorded_estimated_cost_cents: Some(0),
            },
        )?;
        self.upsert_record(
            &agent_record_id(DEMO_AGENT_ID),
            AgentDoc {
                agent_id: DEMO_AGENT_ID.to_owned(),
                company_id: DEMO_COMPANY_ID.to_owned(),
                name: "AxiomNexus Operator".to_owned(),
                role: "implementer".to_owned(),
                status: agent_status_label(AgentStatus::Active).to_owned(),
            },
        )?;
        self.upsert_record(
            &contract_record_id(DEMO_CONTRACT_SET_ID, contract.revision),
            ContractRevisionDoc {
                contract_set_id: DEMO_CONTRACT_SET_ID.to_owned(),
                company_id: DEMO_COMPANY_ID.to_owned(),
                revision: contract.revision,
                name: contract.name.clone(),
                status: contract_status_label(ContractSetStatus::Active).to_owned(),
                rules_json: serde_json::to_value(contract.rules).map_err(|error| {
                    unavailable(&format!("surreal demo contract encode failed: {error}"))
                })?,
            },
        )?;
        self.upsert_record(
            &work_record_id(DEMO_TODO_WORK_ID),
            WorkDoc {
                work_id: DEMO_TODO_WORK_ID.to_owned(),
                company_id: DEMO_COMPANY_ID.to_owned(),
                parent_id: None,
                kind: work_kind_label(WorkKind::Task).to_owned(),
                title: "Todo work".to_owned(),
                body: String::new(),
                status: work_status_label(WorkStatus::Todo).to_owned(),
                priority: priority_label(Priority::Medium).to_owned(),
                assignee_agent_id: None,
                active_lease_id: None,
                rev: 0,
                contract_set_id: DEMO_CONTRACT_SET_ID.to_owned(),
                contract_rev: contract.revision,
                created_at_secs: 0,
                updated_at_secs: 0,
            },
        )?;
        self.upsert_record(
            &work_record_id(DEMO_DOING_WORK_ID),
            WorkDoc {
                work_id: DEMO_DOING_WORK_ID.to_owned(),
                company_id: DEMO_COMPANY_ID.to_owned(),
                parent_id: None,
                kind: work_kind_label(WorkKind::Task).to_owned(),
                title: "Doing work".to_owned(),
                body: String::new(),
                status: work_status_label(WorkStatus::Doing).to_owned(),
                priority: priority_label(Priority::High).to_owned(),
                assignee_agent_id: Some(DEMO_AGENT_ID.to_owned()),
                active_lease_id: Some(DEMO_LEASE_ID.to_owned()),
                rev: 1,
                contract_set_id: DEMO_CONTRACT_SET_ID.to_owned(),
                contract_rev: contract.revision,
                created_at_secs: 0,
                updated_at_secs: 1,
            },
        )?;
        self.upsert_record(
            &lease_record_id(DEMO_LEASE_ID),
            LeaseDoc {
                lease_id: DEMO_LEASE_ID.to_owned(),
                company_id: DEMO_COMPANY_ID.to_owned(),
                work_id: DEMO_DOING_WORK_ID.to_owned(),
                agent_id: DEMO_AGENT_ID.to_owned(),
                run_id: Some("run-1".to_owned()),
                acquired_at_secs: 1,
                expires_at_secs: None,
                released_at_secs: None,
                release_reason: None,
            },
        )?;
        self.upsert_record(
            &run_record_id("run-1"),
            RunDoc {
                run_id: "run-1".to_owned(),
                company_id: DEMO_COMPANY_ID.to_owned(),
                agent_id: DEMO_AGENT_ID.to_owned(),
                work_id: DEMO_DOING_WORK_ID.to_owned(),
                status: run_status_label(RunStatus::Running).to_owned(),
                created_at_secs: 1,
                updated_at_secs: 1,
            },
        )?;
        self.persist_run_activity(
            &self
                .select_record::<RunDoc>(&run_record_id("run-1"))?
                .ok_or_else(|| unavailable("surreal demo run should exist after seed"))?,
        )?;

        Ok(())
    }

    fn export_snapshot_envelope(&self) -> Result<StoreSnapshotEnvelope, StoreError> {
        let state = self.snapshot_state()?;
        let state_bytes = serde_json::to_vec(&state)
            .map_err(|error| unavailable(&format!("surreal snapshot encode failed: {error}")))?;
        Ok(StoreSnapshotEnvelope {
            format: SNAPSHOT_FORMAT.to_owned(),
            checksum_fnv64: fnv64_hex(&state_bytes),
            exported_at_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            state,
        })
    }

    fn snapshot_state(&self) -> Result<StoreSnapshotState, StoreError> {
        let mut state = StoreSnapshotState {
            store_meta: self.load_store_meta()?,
            companies: self.select_table::<CompanyDoc>("company")?,
            agents: self.select_table::<AgentDoc>("agent")?,
            contracts: self.select_table::<ContractRevisionDoc>("contract_revision")?,
            work_items: self.select_table::<WorkDoc>("work")?,
            leases: self.select_table::<LeaseDoc>("lease")?,
            pending_wakes: self.select_table::<PendingWakeDoc>("pending_wake")?,
            runs: self.select_table::<RunDoc>("run")?,
            sessions: self.select_table::<SessionDoc>("task_session")?,
            transition_records: self.select_table::<TransitionRecordDoc>("transition_record")?,
            comments: self.select_table::<WorkCommentDoc>("work_comment")?,
            consumption_events: self.select_table::<ConsumptionEventDoc>("consumption_event")?,
            activity_events: self.select_table::<ActivityEventDoc>("activity_event")?,
        };
        state.normalize();
        Ok(state)
    }
}

impl SurrealStore {
    fn replace_from_snapshot(&self, state: &StoreSnapshotState) -> Result<(), StoreError> {
        self.runtime.block_on(async {
            self.db
                .query(concat!(
                    "BEGIN TRANSACTION;\n",
                    "DELETE company;\n",
                    "DELETE agent;\n",
                    "DELETE contract_revision;\n",
                    "DELETE work;\n",
                    "DELETE lease;\n",
                    "DELETE pending_wake;\n",
                    "DELETE run;\n",
                    "DELETE task_session;\n",
                    "DELETE transition_record;\n",
                    "DELETE work_comment;\n",
                    "DELETE consumption_event;\n",
                    "DELETE activity_event;\n",
                    "DELETE store_meta;\n",
                    "COMMIT TRANSACTION;"
                ))
                .await
                .map_err(|error| {
                    unavailable(&format!("surreal snapshot import reset failed: {error}"))
                })?
                .check()
                .map_err(|error| {
                    unavailable(&format!(
                        "surreal snapshot import reset check failed: {error}"
                    ))
                })?;
            Ok::<(), StoreError>(())
        })?;

        self.save_store_meta(&state.store_meta)?;

        for company in &state.companies {
            self.upsert_record(&company_record_id(&company.company_id), company.clone())?;
        }

        for agent in &state.agents {
            self.upsert_record(&agent_record_id(&agent.agent_id), agent.clone())?;
        }

        for contract in &state.contracts {
            self.upsert_record(
                &contract_record_id(&contract.contract_set_id, contract.revision),
                contract.clone(),
            )?;
        }

        for snapshot in &state.work_items {
            self.upsert_record(&work_record_id(&snapshot.work_id), snapshot.clone())?;
        }

        for lease in &state.leases {
            self.upsert_record(&lease_record_id(&lease.lease_id), lease.clone())?;
        }

        for wake in &state.pending_wakes {
            self.upsert_record(&pending_wake_record_id(&wake.work_id), wake.clone())?;
        }

        for run in &state.runs {
            self.upsert_record(&run_record_id(&run.run_id), run.clone())?;
        }

        for session in &state.sessions {
            self.upsert_record(
                &session_record_id_parts(&session.agent_id, &session.work_id),
                session.clone(),
            )?;
        }

        for comment in &state.comments {
            self.upsert_record(
                &work_comment_record_id(&comment.comment_id),
                comment.clone(),
            )?;
        }

        for event in &state.consumption_events {
            self.upsert_record(&consumption_record_id(&event.event_id), event.clone())?;
        }

        for record in &state.transition_records {
            self.upsert_record(&transition_record_id(&record.record_id), record.clone())?;
        }

        for event in &state.activity_events {
            self.upsert_record(&activity_event_record_id(&event.event_id), event.clone())?;
        }

        Ok(())
    }

    fn save_store_meta(&self, meta: &StoreMeta) -> Result<(), StoreError> {
        self.upsert_record(STORE_META_RECORD, meta.clone())
    }

    fn update_store_meta<T>(&self, f: impl FnOnce(&mut StoreMeta) -> T) -> Result<T, StoreError> {
        let mut meta = self.load_store_meta()?;
        let output = f(&mut meta);
        self.save_store_meta(&meta)?;
        Ok(output)
    }

    fn select_record<T>(&self, record: &str) -> Result<Option<T>, StoreError>
    where
        T: SurrealValue,
    {
        let (table, id) = split_record_id(record)?;
        let table = table.to_owned();
        let id = id.to_owned();
        self.runtime.block_on(async {
            let mut response = self
                .db
                .query("SELECT * FROM type::record($table, $id)")
                .bind(("table", table))
                .bind(("id", id))
                .await
                .map_err(|error| unavailable(&format!("surreal select failed: {error}")))?;

            response
                .take(0)
                .map_err(|error| unavailable(&format!("surreal select decode failed: {error}")))
        })
    }

    fn select_table<T>(&self, table: &str) -> Result<Vec<T>, StoreError>
    where
        T: SurrealValue,
    {
        self.runtime.block_on(async {
            self.db
                .select(table)
                .await
                .map_err(|error| unavailable(&format!("surreal table select failed: {error}")))
        })
    }

    fn query_docs<T>(&self, query: &str) -> Result<Vec<T>, StoreError>
    where
        T: SurrealValue,
    {
        let query = query.to_owned();
        self.runtime.block_on(async {
            let mut response = self
                .db
                .query(query)
                .await
                .map_err(|error| unavailable(&format!("surreal query failed: {error}")))?
                .check()
                .map_err(|error| unavailable(&format!("surreal query check failed: {error}")))?;

            response
                .take(0)
                .map_err(|error| unavailable(&format!("surreal query decode failed: {error}")))
        })
    }

    fn query_docs_with_bind<T, B>(
        &self,
        query: &str,
        bind_name: &str,
        bind_value: B,
    ) -> Result<Vec<T>, StoreError>
    where
        T: SurrealValue,
        B: SurrealValue,
    {
        let query = query.to_owned();
        let bind_name = bind_name.to_owned();
        self.runtime.block_on(async {
            let mut response = self
                .db
                .query(query)
                .bind((bind_name, bind_value))
                .await
                .map_err(|error| unavailable(&format!("surreal query failed: {error}")))?
                .check()
                .map_err(|error| unavailable(&format!("surreal query check failed: {error}")))?;

            response
                .take(0)
                .map_err(|error| unavailable(&format!("surreal query decode failed: {error}")))
        })
    }

    fn query_docs_with_binds<T, B1, B2>(
        &self,
        query: &str,
        left: (&'static str, B1),
        right: (&'static str, B2),
    ) -> Result<Vec<T>, StoreError>
    where
        T: SurrealValue,
        B1: SurrealValue,
        B2: SurrealValue,
    {
        let query = query.to_owned();
        self.runtime.block_on(async {
            let mut response = self
                .db
                .query(query)
                .bind(left)
                .bind(right)
                .await
                .map_err(|error| unavailable(&format!("surreal query failed: {error}")))?
                .check()
                .map_err(|error| unavailable(&format!("surreal query check failed: {error}")))?;

            response
                .take(0)
                .map_err(|error| unavailable(&format!("surreal query decode failed: {error}")))
        })
    }

    fn upsert_record<T>(&self, record: &str, doc: T) -> Result<(), StoreError>
    where
        T: SurrealValue,
    {
        let (table, id) = split_record_id(record)?;
        let table = table.to_owned();
        let id = id.to_owned();
        self.runtime.block_on(async {
            self.db
                .query("UPSERT type::record($table, $id) CONTENT $content")
                .bind(("table", table))
                .bind(("id", id))
                .bind(("content", doc))
                .await
                .map_err(|error| unavailable(&format!("surreal upsert failed: {error}")))?
                .check()
                .map_err(|error| unavailable(&format!("surreal upsert check failed: {error}")))?;
            Ok(())
        })
    }
}

fn split_record_id(record: &str) -> Result<(&str, &str), StoreError> {
    record
        .split_once(':')
        .ok_or_else(|| unavailable("surreal record id must include table prefix"))
}

impl Default for StoreMeta {
    fn default() -> Self {
        Self {
            next_company_seq: 3,
            next_agent_seq: 6,
            next_contract_seq: 14,
            next_work_seq: 12,
            next_run_seq: 1,
            next_lease_seq: 32,
            next_comment_seq: 0,
            next_consumption_seq: 0,
            tick: 10,
        }
    }
}

impl StorePort for SurrealStore {
    fn append_comment(&self, req: AppendCommentReq) -> Result<AppendCommentRes, StoreError> {
        let snapshot = self
            .select_record::<WorkDoc>(&work_record_id(req.work_id.as_str()))?
            .ok_or_else(|| not_found("append_comment", req.work_id.as_str()))?;

        if snapshot.company_id != req.company_id.as_str() {
            return Err(conflict(
                "append_comment rejects company/work boundary violations",
            ));
        }

        let comment_id = self.append_work_comment(
            &req.company_id,
            &req.work_id,
            req.author_kind,
            &req.author_id,
            None,
            req.body,
        )?;

        Ok(AppendCommentRes { comment_id })
    }

    fn create_agent(&self, req: CreateAgentReq) -> Result<CreateAgentRes, StoreError> {
        if self
            .select_record::<CompanyDoc>(&company_record_id(req.company_id.as_str()))?
            .is_none()
        {
            return Err(conflict("create_agent requires a registered company"));
        }

        let agent_id = self.update_store_meta(|meta| {
            meta.next_agent_seq += 1;
            AgentId::from(sequenced_id(meta.next_agent_seq))
        })?;

        self.upsert_record(
            &agent_record_id(agent_id.as_str()),
            AgentDoc {
                agent_id: agent_id.to_string(),
                company_id: req.company_id.to_string(),
                name: req.name,
                role: req.role,
                status: agent_status_label(AgentStatus::Active).to_owned(),
            },
        )?;

        Ok(CreateAgentRes {
            agent_id,
            status: AgentStatus::Active,
        })
    }

    fn create_company(&self, req: CreateCompanyReq) -> Result<CreateCompanyRes, StoreError> {
        let profile = self.update_store_meta(|meta| {
            meta.next_company_seq += 1;
            CompanyProfile {
                company_id: CompanyId::from(sequenced_id(meta.next_company_seq)),
                name: req.name,
                description: req.description,
                runtime_hard_stop_cents: req.runtime_hard_stop_cents,
                recorded_estimated_cost_cents: 0,
            }
        })?;

        self.upsert_record(
            &company_record_id(profile.company_id.as_str()),
            CompanyDoc {
                company_id: profile.company_id.to_string(),
                name: profile.name.clone(),
                description: profile.description.clone(),
                runtime_hard_stop_cents: profile.runtime_hard_stop_cents,
                recorded_estimated_cost_cents: Some(profile.recorded_estimated_cost_cents),
            },
        )?;

        Ok(CreateCompanyRes { profile })
    }

    fn create_work(&self, req: CreateWorkReq) -> Result<CreateWorkRes, StoreError> {
        let active_contract = active_contract_for_company(self, &req.company_id)?
            .ok_or_else(|| conflict("create_work requires an active contract for the company"))?;

        if active_contract.contract_set_id != req.contract_set_id {
            return Err(conflict(
                "create_work requires the active contract set for the company",
            ));
        }

        if let Some(parent_id) = req.parent_id.as_ref() {
            let parent = self
                .select_record::<WorkDoc>(&work_record_id(parent_id.as_str()))?
                .ok_or_else(|| not_found("create_work parent", parent_id.as_str()))?;
            if parent.company_id != req.company_id.as_str() {
                return Err(conflict(
                    "create_work rejects parent/company boundary violations",
                ));
            }
            if parent.contract_set_id != req.contract_set_id.as_str() {
                return Err(conflict(
                    "create_work requires parent and child to share contract set",
                ));
            }
        }

        let (work_id, timestamp_secs) = self.update_store_meta(|meta| {
            meta.next_work_seq += 1;
            meta.tick += 1;
            (WorkId::from(sequenced_id(meta.next_work_seq)), meta.tick)
        })?;

        let snapshot = WorkSnapshot {
            work_id: work_id.clone(),
            company_id: req.company_id,
            parent_id: req.parent_id,
            kind: req.kind,
            title: req.title,
            body: req.body,
            status: WorkStatus::Backlog,
            priority: Priority::Medium,
            assignee_agent_id: None,
            active_lease_id: None,
            rev: 0,
            contract_set_id: req.contract_set_id,
            contract_rev: active_contract.revision,
            created_at: timestamp(timestamp_secs),
            updated_at: timestamp(timestamp_secs),
        };

        self.upsert_record(
            &work_record_id(snapshot.work_id.as_str()),
            WorkDoc::from_snapshot(&snapshot),
        )?;

        Ok(CreateWorkRes { snapshot })
    }

    fn set_agent_status(&self, req: SetAgentStatusReq) -> Result<SetAgentStatusRes, StoreError> {
        let existing = self
            .select_record::<AgentDoc>(&agent_record_id(req.agent_id.as_str()))?
            .ok_or_else(|| not_found("set_agent_status", req.agent_id.as_str()))?;
        let current = parse_agent_status(&existing.status)?;

        if current == AgentStatus::Terminated && req.status != AgentStatus::Terminated {
            return Err(conflict("terminated agent cannot resume"));
        }

        self.upsert_record(
            &agent_record_id(req.agent_id.as_str()),
            AgentDoc {
                status: agent_status_label(req.status).to_owned(),
                ..existing
            },
        )?;

        Ok(SetAgentStatusRes {
            agent_id: req.agent_id,
            status: req.status,
        })
    }

    fn update_work(&self, req: UpdateWorkReq) -> Result<UpdateWorkRes, StoreError> {
        let existing = self
            .select_record::<WorkDoc>(&work_record_id(req.work_id.as_str()))?
            .ok_or_else(|| not_found("update_work", req.work_id.as_str()))?;

        if let Some(parent_id) = req.parent_id.as_ref() {
            let parent = self
                .select_record::<WorkDoc>(&work_record_id(parent_id.as_str()))?
                .ok_or_else(|| not_found("update_work parent", parent_id.as_str()))?;
            if parent.company_id != existing.company_id {
                return Err(conflict(
                    "update_work rejects parent/company boundary violations",
                ));
            }
            if parent.contract_set_id != existing.contract_set_id {
                return Err(conflict(
                    "update_work requires parent and child to share contract set",
                ));
            }
            if parent_id == &req.work_id
                || would_create_work_cycle(self, req.work_id.as_str(), parent_id.as_str())?
            {
                return Err(conflict("update_work rejects tree cycles"));
            }
        }

        let updated_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;

        let updated = WorkDoc {
            work_id: existing.work_id.clone(),
            company_id: existing.company_id.clone(),
            parent_id: req.parent_id.as_ref().map(ToString::to_string),
            kind: existing.kind.clone(),
            title: req.title,
            body: req.body,
            status: existing.status.clone(),
            priority: existing.priority.clone(),
            assignee_agent_id: existing.assignee_agent_id.clone(),
            active_lease_id: existing.active_lease_id.clone(),
            rev: existing.rev + 1,
            contract_set_id: existing.contract_set_id.clone(),
            contract_rev: existing.contract_rev,
            created_at_secs: existing.created_at_secs,
            updated_at_secs,
        };

        self.upsert_record(&work_record_id(req.work_id.as_str()), updated.clone())?;

        Ok(UpdateWorkRes {
            snapshot: updated.into_snapshot()?,
        })
    }

    fn create_contract_draft(
        &self,
        req: CreateContractDraftReq,
    ) -> Result<CreateContractDraftRes, StoreError> {
        if self
            .select_record::<CompanyDoc>(&company_record_id(req.company_id.as_str()))?
            .is_none()
        {
            return Err(conflict(
                "create_contract_draft requires a registered company",
            ));
        }

        let existing = contracts_for_company(self, &req.company_id)?;
        let revision = existing
            .iter()
            .map(|contract| contract.revision)
            .max()
            .unwrap_or(0)
            + 1;
        let contract_set_id = existing
            .iter()
            .max_by_key(|contract| contract.revision)
            .map(|contract| contract.contract_set_id.clone())
            .unwrap_or_else(|| {
                self.update_store_meta(|meta| {
                    meta.next_contract_seq += 1;
                    sequenced_id(meta.next_contract_seq)
                })
                .expect("contract id allocation should succeed")
            });

        let draft = ContractRevisionDoc {
            contract_set_id: contract_set_id.clone(),
            company_id: req.company_id.to_string(),
            revision,
            name: req.name,
            status: contract_status_label(ContractSetStatus::Draft).to_owned(),
            rules_json: serde_json::to_value(req.rules)
                .map_err(|error| unavailable(&format!("contract rule encode failed: {error}")))?,
        };

        self.upsert_record(&contract_record_id(&contract_set_id, revision), draft)?;

        Ok(CreateContractDraftRes { revision })
    }

    fn activate_contract(
        &self,
        req: ActivateContractReq,
    ) -> Result<ActivateContractRes, StoreError> {
        if self
            .select_record::<CompanyDoc>(&company_record_id(req.company_id.as_str()))?
            .is_none()
        {
            return Err(conflict("activate_contract requires a registered company"));
        }

        let contracts = contracts_for_company(self, &req.company_id)?;
        let mut found = false;
        for contract in contracts {
            let next_status = if contract.revision == req.revision {
                found = true;
                ContractSetStatus::Active
            } else if contract.status == contract_status_label(ContractSetStatus::Active) {
                ContractSetStatus::Retired
            } else {
                parse_contract_status(&contract.status)?
            };

            self.upsert_record(
                &contract_record_id(&contract.contract_set_id, contract.revision),
                ContractRevisionDoc {
                    status: contract_status_label(next_status).to_owned(),
                    ..contract
                },
            )?;
        }

        if !found {
            return Err(not_found(
                "activate_contract revision",
                &req.revision.to_string(),
            ));
        }

        Ok(ActivateContractRes {
            revision: req.revision,
        })
    }

    fn claim_lease(&self, req: ClaimLeaseReq) -> Result<ClaimLeaseRes, StoreError> {
        SurrealStore::claim_lease(self, req)
    }

    fn load_context(&self, work_id: &WorkId) -> Result<WorkContext, StoreError> {
        let snapshot_doc = self
            .select_record::<WorkDoc>(&work_record_id(work_id.as_str()))?
            .ok_or_else(|| not_found("load_context", work_id.as_str()))?;
        let snapshot = snapshot_doc.clone().into_snapshot()?;
        let lease = snapshot_doc
            .active_lease_id
            .as_ref()
            .and_then(|lease_id| {
                self.select_record::<LeaseDoc>(&lease_record_id(lease_id))
                    .ok()
            })
            .flatten()
            .filter(|lease| lease.released_at_secs.is_none())
            .map(LeaseDoc::into_model)
            .transpose()?;
        let pending_wake = self
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(work_id.as_str()))?
            .map(PendingWakeDoc::into_model)
            .transpose()?;
        let contract = contract_for_work(self, &snapshot_doc)?;

        Ok(WorkContext {
            snapshot,
            lease,
            pending_wake,
            contract,
        })
    }

    fn list_work_snapshots(&self) -> Result<Vec<WorkSnapshot>, StoreError> {
        let mut snapshots = self
            .select_table::<WorkDoc>("work")?
            .into_iter()
            .map(WorkDoc::into_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        snapshots.sort_by(|left, right| left.work_id.cmp(&right.work_id));
        Ok(snapshots)
    }

    fn load_transition_records(
        &self,
        work_id: &WorkId,
    ) -> Result<Vec<TransitionRecord>, StoreError> {
        self.query_docs_with_bind::<TransitionRecordDoc, _>(
            "SELECT * FROM transition_record WHERE work_id = $work_id ORDER BY expected_rev ASC, happened_at_secs ASC, record_id ASC",
            "work_id",
            work_id.as_str().to_owned(),
        )?
        .into_iter()
        .map(TransitionRecordDoc::into_model)
        .collect()
    }

    fn merge_wake(&self, req: MergeWakeReq) -> Result<crate::model::PendingWake, StoreError> {
        let snapshot = self
            .select_record::<WorkDoc>(&work_record_id(req.work_id.as_str()))?
            .ok_or_else(|| not_found("merge_wake", req.work_id.as_str()))?;
        let existing = self
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(req.work_id.as_str()))?
            .map(PendingWakeDoc::into_model)
            .transpose()?;
        let merged_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;
        let merged = kernel::merge_wake(
            existing.as_ref(),
            &req.reason,
            &req.obligations,
            timestamp(merged_at_secs),
            req.work_id.clone(),
        );

        self.upsert_record(
            &pending_wake_record_id(req.work_id.as_str()),
            PendingWakeDoc::from_model(&merged),
        )?;
        self.append_work_comment(
            &CompanyId::from(snapshot.company_id.clone()),
            &req.work_id,
            req.actor_kind,
            &req.actor_id,
            Some(req.source.clone()),
            wake_comment_body(&req.source, &req.reason, &req.obligations, merged.count),
        )?;
        let _ = self.ensure_wake_runnable_run(&snapshot, merged_at_secs)?;

        Ok(merged)
    }

    fn reap_timed_out_runs(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Vec<ReapedRun>, StoreError> {
        let reaped_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;
        let stale_runs = self.query_docs_with_binds::<RunDoc, _, _>(
            "SELECT * FROM run WHERE status = $status AND updated_at_secs <= $cutoff ORDER BY updated_at_secs ASC",
            ("status", run_status_label(RunStatus::Running).to_owned()),
            ("cutoff", reaped_at_secs.saturating_sub(timeout.as_secs())),
        )?;

        let mut reaped = Vec::new();
        for stale_run in stale_runs {
            let failed_run = RunDoc {
                status: run_status_label(RunStatus::TimedOut).to_owned(),
                updated_at_secs: reaped_at_secs,
                ..stale_run.clone()
            };
            self.upsert_record(&run_record_id(&stale_run.run_id), failed_run.clone())?;
            self.persist_run_activity(&failed_run)?;

            let Some(snapshot_doc) =
                self.select_record::<WorkDoc>(&work_record_id(&stale_run.work_id))?
            else {
                continue;
            };
            let released_lease_id =
                self.find_open_lease_for_run(&stale_run.work_id, &stale_run.run_id)?;
            if let Some(lease_id) = released_lease_id.as_ref() {
                self.commit_decision(timeout_requeue_commit_req(
                    &snapshot_doc,
                    &stale_run,
                    lease_id,
                    reaped_at_secs,
                ))?;
            }
            let follow_up_run_id = if self
                .select_record::<PendingWakeDoc>(&pending_wake_record_id(&stale_run.work_id))?
                .is_some()
            {
                let snapshot = self
                    .select_record::<WorkDoc>(&work_record_id(&stale_run.work_id))?
                    .ok_or_else(|| not_found("reap_timed_out_runs snapshot", &stale_run.work_id))?;
                self.ensure_wake_runnable_run(&snapshot, reaped_at_secs)?
                    .map(|run| RunId::from(run.run_id))
            } else {
                None
            };
            self.append_work_comment(
                &CompanyId::from(stale_run.company_id.clone()),
                &WorkId::from(stale_run.work_id.clone()),
                ActorKind::System,
                &crate::model::ActorId::from("system"),
                Some("scheduler".to_owned()),
                reaper_comment_body(
                    &RunId::from(stale_run.run_id.clone()),
                    released_lease_id.as_ref(),
                    follow_up_run_id.as_ref(),
                ),
            )?;

            reaped.push(ReapedRun {
                run_id: RunId::from(stale_run.run_id),
                work_id: WorkId::from(stale_run.work_id),
                released_lease_id,
                follow_up_run_id,
            });
        }

        Ok(reaped)
    }

    fn load_queued_runs(&self) -> Result<Vec<QueuedRunCandidate>, StoreError> {
        let runs = self.query_docs::<RunDoc>(
            "SELECT * FROM run WHERE status = 'queued' ORDER BY created_at_secs ASC",
        )?;
        let agents = self.query_docs::<AgentDoc>("SELECT * FROM agent")?;
        let companies = self.query_docs::<CompanyDoc>("SELECT * FROM company")?;
        let agent_statuses = agents
            .into_iter()
            .map(|agent| (agent.agent_id, parse_agent_status(&agent.status)))
            .collect::<Vec<_>>();
        let company_budget_state = companies
            .into_iter()
            .map(|company| {
                (
                    company.company_id,
                    (
                        company.runtime_hard_stop_cents,
                        company.recorded_estimated_cost_cents.unwrap_or(0),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();

        Ok(runs
            .into_iter()
            .map(|run| QueuedRunCandidate {
                run_id: RunId::from(run.run_id),
                agent_status: agent_statuses
                    .iter()
                    .find(|(agent_id, _)| agent_id == &run.agent_id)
                    .and_then(|(_, status)| status.as_ref().ok().copied()),
                budget_blocked: company_budget_hard_stopped_from_rollup(
                    company_budget_state
                        .get(run.company_id.as_str())
                        .copied()
                        .unwrap_or((None, 0)),
                ),
                created_at: timestamp(run.created_at_secs),
            })
            .collect())
    }

    fn load_runtime_turn(
        &self,
        run_id: &crate::model::RunId,
    ) -> Result<RuntimeTurnContext, StoreError> {
        let run = self
            .select_record::<RunDoc>(&run_record_id(run_id.as_str()))?
            .ok_or_else(|| not_found("load_runtime_turn", run_id.as_str()))?;

        if !parse_run_status(&run.status)?.is_runnable() {
            return Err(conflict(
                "load_runtime_turn requires a queued or running run",
            ));
        }

        let agent = self
            .select_record::<AgentDoc>(&agent_record_id(&run.agent_id))?
            .ok_or_else(|| conflict("load_runtime_turn requires a registered agent"))?;
        if parse_agent_status(&agent.status)? != AgentStatus::Active {
            return Err(conflict(
                "load_runtime_turn requires an active agent for queued or running run",
            ));
        }

        let snapshot_doc = self
            .select_record::<WorkDoc>(&work_record_id(&run.work_id))?
            .ok_or_else(|| not_found("load_runtime_turn snapshot", &run.work_id))?;
        let pending_wake = self
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(&run.work_id))?
            .map(PendingWakeDoc::into_model)
            .transpose()?;
        let contract = contract_for_work(self, &snapshot_doc)?;

        Ok(RuntimeTurnContext {
            run_id: RunId::from(run.run_id),
            agent_id: AgentId::from(run.agent_id),
            snapshot: snapshot_doc.into_snapshot()?,
            pending_wake,
            contract,
        })
    }

    fn load_agent_facts(&self, agent_id: &AgentId) -> Result<Option<AgentFacts>, StoreError> {
        let agent = self.select_record::<AgentDoc>(&agent_record_id(agent_id.as_str()))?;
        let Some(agent) = agent else {
            return Ok(None);
        };

        Ok(Some(AgentFacts {
            company_id: CompanyId::from(agent.company_id),
            status: parse_agent_status(&agent.status)?,
        }))
    }

    fn mark_run_running(&self, run_id: &crate::model::RunId) -> Result<(), StoreError> {
        let run = self
            .select_record::<RunDoc>(&run_record_id(run_id.as_str()))?
            .ok_or_else(|| not_found("mark_run_running", run_id.as_str()))?;

        if !parse_run_status(&run.status)?.is_runnable() {
            return Err(conflict(
                "mark_run_running requires a queued or running run",
            ));
        }

        let agent = self
            .select_record::<AgentDoc>(&agent_record_id(&run.agent_id))?
            .ok_or_else(|| conflict("mark_run_running requires a registered agent"))?;
        if parse_agent_status(&agent.status)? != AgentStatus::Active {
            return Err(conflict("mark_run_running requires an active agent"));
        }

        let updated_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;

        let should_record_activity = run.status != run_status_label(RunStatus::Running);
        let running = RunDoc {
            status: run_status_label(RunStatus::Running).to_owned(),
            updated_at_secs,
            ..run
        };

        self.upsert_record(&run_record_id(run_id.as_str()), running.clone())?;
        if should_record_activity {
            self.persist_run_activity(&running)?;
        }

        Ok(())
    }

    fn mark_run_completed(&self, run_id: &crate::model::RunId) -> Result<(), StoreError> {
        let run = self
            .select_record::<RunDoc>(&run_record_id(run_id.as_str()))?
            .ok_or_else(|| not_found("mark_run_completed", run_id.as_str()))?;

        if !parse_run_status(&run.status)?.is_runnable() {
            return Err(conflict(
                "mark_run_completed requires a queued or running run",
            ));
        }

        let updated_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;

        let completed = RunDoc {
            status: run_status_label(RunStatus::Completed).to_owned(),
            updated_at_secs,
            ..run
        };

        self.upsert_record(&run_record_id(run_id.as_str()), completed.clone())?;
        self.persist_run_completion_activity(&completed)?;

        Ok(())
    }

    fn mark_run_failed(
        &self,
        run_id: &crate::model::RunId,
        reason: &str,
    ) -> Result<(), StoreError> {
        let run = self
            .select_record::<RunDoc>(&run_record_id(run_id.as_str()))?
            .ok_or_else(|| not_found("mark_run_failed", run_id.as_str()))?;

        if !parse_run_status(&run.status)?.is_runnable() {
            return Err(conflict("mark_run_failed requires a queued or running run"));
        }

        let updated_at_secs = self.update_store_meta(|meta| {
            meta.tick += 1;
            meta.tick
        })?;

        let failed = RunDoc {
            status: run_status_label(RunStatus::Failed).to_owned(),
            updated_at_secs,
            ..run
        };

        self.upsert_record(&run_record_id(run_id.as_str()), failed.clone())?;
        self.persist_run_failure_activity(&failed, reason)?;

        Ok(())
    }

    fn load_session(
        &self,
        key: &SessionKey,
    ) -> Result<Option<crate::model::TaskSession>, StoreError> {
        self.select_record::<SessionDoc>(&session_record_id(key))
            .and_then(|session| session.map(SessionDoc::into_model).transpose())
    }

    fn save_session(&self, session: &crate::model::TaskSession) -> Result<(), StoreError> {
        self.upsert_record(
            &session_record_id_parts(session.agent_id.as_str(), session.work_id.as_str()),
            SessionDoc::from_model(session),
        )
    }

    fn record_consumption(&self, req: RecordConsumptionReq) -> Result<(), StoreError> {
        let run = self
            .select_record::<RunDoc>(&run_record_id(req.run_id.as_str()))?
            .ok_or_else(|| not_found("record_consumption", req.run_id.as_str()))?;

        if run.company_id != req.company_id.as_str() || run.agent_id != req.agent_id.as_str() {
            return Err(conflict(
                "record_consumption rejects company/run or agent/run boundary violations",
            ));
        }
        let company = self
            .select_record::<CompanyDoc>(&company_record_id(&run.company_id))?
            .ok_or_else(|| not_found("record_consumption company", &run.company_id))?;

        let (event_id, created_at_secs) = self.update_store_meta(|meta| {
            meta.next_consumption_seq += 1;
            meta.tick += 1;
            (
                format!("consumption-{}", meta.next_consumption_seq),
                meta.tick,
            )
        })?;

        self.upsert_record(
            &consumption_record_id(&event_id),
            ConsumptionEventDoc {
                event_id,
                company_id: req.company_id.to_string(),
                agent_id: req.agent_id.to_string(),
                run_id: req.run_id.to_string(),
                billing_kind: billing_kind_label(req.billing_kind).to_owned(),
                input_tokens: req.usage.input_tokens,
                output_tokens: req.usage.output_tokens,
                run_seconds: req.usage.run_seconds,
                estimated_cost_cents: req.usage.estimated_cost_cents,
                created_at_secs,
            },
        )?;
        self.upsert_record(
            &company_record_id(req.company_id.as_str()),
            CompanyDoc {
                recorded_estimated_cost_cents: Some(
                    company.recorded_estimated_cost_cents.unwrap_or(0)
                        + req.usage.estimated_cost_cents.unwrap_or(0),
                ),
                ..company
            },
        )
    }

    fn commit_decision(&self, req: CommitDecisionReq) -> Result<CommitDecisionRes, StoreError> {
        let _commit_guard = self
            .commit_lock
            .lock()
            .map_err(|_| unavailable("surreal commit lock is poisoned"))?;
        let authoritative = commit::load_commit_authoritative_state(self, &req)?;
        let prepared = commit::prepare_commit_decision(self, req, authoritative)?;
        commit::execute_commit_decision_transaction(self, &prepared)?;
        commit::load_commit_decision_result(self, prepared)
    }

    fn read_board(&self) -> BoardReadModel {
        query::read_board(self)
    }

    fn read_companies(&self) -> CompanyReadModel {
        query::read_companies(self)
    }

    fn read_work(&self, work_id: Option<&WorkId>) -> Result<WorkReadModel, StoreError> {
        query::read_work(self, work_id)
    }

    fn read_agents(&self) -> AgentReadModel {
        query::read_agents(self)
    }

    fn read_activity(&self) -> ActivityReadModel {
        query::read_activity(self)
    }

    fn read_run(&self, run_id: &crate::model::RunId) -> Result<RunReadModel, StoreError> {
        query::read_run(self, run_id)
    }

    fn read_contracts(&self) -> ContractsReadModel {
        query::read_contracts(self)
    }
}

impl WorkDoc {
    fn from_snapshot(snapshot: &WorkSnapshot) -> Self {
        Self {
            work_id: snapshot.work_id.to_string(),
            company_id: snapshot.company_id.to_string(),
            parent_id: snapshot.parent_id.as_ref().map(ToString::to_string),
            kind: work_kind_label(snapshot.kind).to_owned(),
            title: snapshot.title.clone(),
            body: snapshot.body.clone(),
            status: work_status_label(snapshot.status).to_owned(),
            priority: priority_label(snapshot.priority).to_owned(),
            assignee_agent_id: snapshot.assignee_agent_id.as_ref().map(ToString::to_string),
            active_lease_id: snapshot.active_lease_id.as_ref().map(ToString::to_string),
            rev: snapshot.rev,
            contract_set_id: snapshot.contract_set_id.to_string(),
            contract_rev: snapshot.contract_rev,
            created_at_secs: timestamp_secs(snapshot.created_at),
            updated_at_secs: timestamp_secs(snapshot.updated_at),
        }
    }

    fn into_snapshot(self) -> Result<WorkSnapshot, StoreError> {
        Ok(WorkSnapshot {
            work_id: WorkId::from(self.work_id),
            company_id: CompanyId::from(self.company_id),
            parent_id: self.parent_id.map(WorkId::from),
            kind: parse_work_kind(&self.kind)?,
            title: self.title,
            body: self.body,
            status: parse_work_status(&self.status)?,
            priority: parse_priority(&self.priority)?,
            assignee_agent_id: self.assignee_agent_id.map(AgentId::from),
            active_lease_id: self.active_lease_id.map(crate::model::LeaseId::from),
            rev: self.rev,
            contract_set_id: ContractSetId::from(self.contract_set_id),
            contract_rev: self.contract_rev,
            created_at: timestamp(self.created_at_secs),
            updated_at: timestamp(self.updated_at_secs),
        })
    }
}

impl LeaseDoc {
    fn from_model(lease: &WorkLease) -> Self {
        Self {
            lease_id: lease.lease_id.to_string(),
            company_id: lease.company_id.to_string(),
            work_id: lease.work_id.to_string(),
            agent_id: lease.agent_id.to_string(),
            run_id: lease.run_id.as_ref().map(ToString::to_string),
            acquired_at_secs: timestamp_secs(lease.acquired_at),
            expires_at_secs: lease.expires_at.map(timestamp_secs),
            released_at_secs: lease.released_at.map(timestamp_secs),
            release_reason: lease
                .release_reason
                .map(|reason| lease_release_reason_label(reason).to_owned()),
        }
    }

    fn into_model(self) -> Result<WorkLease, StoreError> {
        Ok(WorkLease {
            lease_id: crate::model::LeaseId::from(self.lease_id),
            company_id: CompanyId::from(self.company_id),
            work_id: WorkId::from(self.work_id),
            agent_id: AgentId::from(self.agent_id),
            run_id: self.run_id.map(RunId::from),
            acquired_at: timestamp(self.acquired_at_secs),
            expires_at: self.expires_at_secs.map(timestamp),
            released_at: self.released_at_secs.map(timestamp),
            release_reason: self
                .release_reason
                .as_deref()
                .map(parse_lease_release_reason)
                .transpose()?,
        })
    }
}

impl PendingWakeDoc {
    fn from_model(wake: &PendingWake) -> Self {
        Self {
            work_id: wake.work_id.to_string(),
            obligations: wake.obligation_json.iter().cloned().collect(),
            count: wake.count,
            latest_reason: wake.latest_reason.clone(),
            merged_at_secs: timestamp_secs(wake.merged_at),
        }
    }

    fn into_model(self) -> Result<PendingWake, StoreError> {
        Ok(PendingWake {
            work_id: WorkId::from(self.work_id),
            obligation_json: self.obligations.into_iter().collect::<BTreeSet<_>>(),
            count: self.count,
            latest_reason: self.latest_reason,
            merged_at: timestamp(self.merged_at_secs),
        })
    }
}

impl SessionDoc {
    fn from_model(session: &TaskSession) -> Self {
        Self {
            session_id: session.session_id.to_string(),
            company_id: session.company_id.to_string(),
            agent_id: session.agent_id.to_string(),
            work_id: session.work_id.to_string(),
            runtime: runtime_kind_label(session.runtime).to_owned(),
            runtime_session_id: session.runtime_session_id.clone(),
            cwd: session.cwd.clone(),
            workspace_fingerprint: session.workspace_fingerprint.clone(),
            contract_rev: session.contract_rev,
            last_record_id: session.last_record_id.as_ref().map(ToString::to_string),
            last_decision_summary: session.last_decision_summary.clone(),
            last_gate_summary: session.last_gate_summary.clone(),
            updated_at_secs: timestamp_secs(session.updated_at),
        }
    }

    fn into_model(self) -> Result<TaskSession, StoreError> {
        Ok(TaskSession {
            session_id: SessionId::from(self.session_id),
            company_id: CompanyId::from(self.company_id),
            agent_id: AgentId::from(self.agent_id),
            work_id: WorkId::from(self.work_id),
            runtime: parse_runtime_kind(&self.runtime)?,
            runtime_session_id: self.runtime_session_id,
            cwd: self.cwd,
            workspace_fingerprint: self.workspace_fingerprint,
            contract_rev: self.contract_rev,
            last_record_id: self.last_record_id.map(crate::model::RecordId::from),
            last_decision_summary: self.last_decision_summary,
            last_gate_summary: self.last_gate_summary,
            updated_at: timestamp(self.updated_at_secs),
        })
    }

    fn into_agent_session_summary(
        self,
    ) -> Result<crate::port::store::AgentSessionSummaryView, StoreError> {
        Ok(crate::port::store::AgentSessionSummaryView {
            agent_id: self.agent_id,
            work_id: self.work_id,
            runtime: parse_runtime_kind(&self.runtime)?,
            runtime_session_id: self.runtime_session_id,
            cwd: self.cwd,
            contract_rev: self.contract_rev,
            last_decision_summary: self.last_decision_summary,
            last_gate_summary: self.last_gate_summary,
        })
    }
}

impl TransitionRecordDoc {
    fn from_model(record: &TransitionRecord) -> Result<Self, StoreError> {
        Ok(Self {
            record_id: record.record_id.to_string(),
            company_id: record.company_id.to_string(),
            work_id: record.work_id.to_string(),
            actor_kind: actor_kind_label(record.actor_kind).to_owned(),
            actor_id: record.actor_id.to_string(),
            run_id: record.run_id.as_ref().map(ToString::to_string),
            session_id: record.session_id.as_ref().map(ToString::to_string),
            lease_id: record.lease_id.as_ref().map(ToString::to_string),
            expected_rev: record.expected_rev,
            contract_set_id: record.contract_set_id.to_string(),
            contract_rev: record.contract_rev,
            before_status: work_status_label(record.before_status).to_owned(),
            after_status: record
                .after_status
                .map(|status| work_status_label(status).to_owned()),
            outcome: decision_outcome_label(record.outcome).to_owned(),
            reasons_json: serde_json::to_value(&record.reasons)
                .map_err(|error| unavailable(&format!("reason encode failed: {error}")))?,
            kind: transition_kind_label(record.kind).to_owned(),
            patch_summary: record.patch.summary.clone(),
            resolved_obligations: record.patch.resolved_obligations.clone(),
            declared_risks: record.patch.declared_risks.clone(),
            failed_gates: store_support::failed_gate_details(&record.gate_results),
            gate_results_json: serde_json::to_value(&record.gate_results)
                .map_err(|error| unavailable(&format!("gate result encode failed: {error}")))?,
            evidence_json: serde_json::to_value(&record.evidence)
                .map_err(|error| unavailable(&format!("evidence encode failed: {error}")))?,
            evidence_summary: record
                .evidence_inline
                .as_ref()
                .map(|evidence| evidence.summary.clone()),
            evidence_refs_json: serde_json::to_value(&record.evidence_refs)
                .map_err(|error| unavailable(&format!("evidence ref encode failed: {error}")))?,
            happened_at_secs: timestamp_secs(record.happened_at),
        })
    }

    fn into_model(self) -> Result<TransitionRecord, StoreError> {
        Ok(TransitionRecord {
            record_id: crate::model::RecordId::from(self.record_id),
            company_id: CompanyId::from(self.company_id),
            work_id: WorkId::from(self.work_id),
            actor_kind: parse_actor_kind(&self.actor_kind)?,
            actor_id: crate::model::ActorId::from(self.actor_id),
            run_id: self.run_id.map(RunId::from),
            session_id: self.session_id.map(SessionId::from),
            lease_id: self.lease_id.map(LeaseId::from),
            expected_rev: self.expected_rev,
            contract_set_id: ContractSetId::from(self.contract_set_id),
            contract_rev: self.contract_rev,
            before_status: parse_work_status(&self.before_status)?,
            after_status: self
                .after_status
                .as_deref()
                .map(parse_work_status)
                .transpose()?,
            outcome: parse_decision_outcome(&self.outcome)?,
            reasons: serde_json::from_value(self.reasons_json)
                .map_err(|error| unavailable(&format!("reason decode failed: {error}")))?,
            kind: parse_transition_kind(&self.kind)?,
            patch: crate::model::WorkPatch {
                summary: self.patch_summary,
                resolved_obligations: self.resolved_obligations,
                declared_risks: self.declared_risks,
            },
            gate_results: serde_json::from_value(self.gate_results_json)
                .map_err(|error| unavailable(&format!("gate result decode failed: {error}")))?,
            evidence: serde_json::from_value(self.evidence_json)
                .map_err(|error| unavailable(&format!("evidence decode failed: {error}")))?,
            evidence_inline: self
                .evidence_summary
                .map(|summary| crate::model::EvidenceInline { summary }),
            evidence_refs: serde_json::from_value(self.evidence_refs_json)
                .map_err(|error| unavailable(&format!("evidence ref decode failed: {error}")))?,
            happened_at: timestamp(self.happened_at_secs),
        })
    }
}

impl ActivityEventDoc {
    fn from_transition_record(record: &TransitionRecord) -> Self {
        let view = store_support::transition_activity_entry_view(record);
        Self {
            event_id: record.record_id.to_string(),
            work_id: view.work_id,
            event_kind: view.event_kind,
            summary: view.summary,
            actor_kind: view
                .actor_kind
                .map(|actor_kind| actor_kind_label(actor_kind).to_owned()),
            actor_id: view.actor_id,
            source: view.source,
            before_status: view
                .before_status
                .map(|status| work_status_label(status).to_owned()),
            after_status: view
                .after_status
                .map(|status| work_status_label(status).to_owned()),
            outcome: view.outcome,
            evidence_summary: view.evidence_summary,
            happened_at_secs: timestamp_secs(record.happened_at),
            priority: 3,
        }
    }

    fn from_comment(comment: &WorkCommentDoc) -> Self {
        Self {
            event_id: comment.comment_id.clone(),
            work_id: comment.work_id.clone(),
            event_kind: "comment".to_owned(),
            summary: comment.body.clone(),
            actor_kind: Some(comment.author_kind.clone()),
            actor_id: Some(comment.author_id.clone()),
            source: comment.source.clone(),
            before_status: None,
            after_status: None,
            outcome: None,
            evidence_summary: None,
            happened_at_secs: comment.created_at_secs,
            priority: 2,
        }
    }

    fn from_run(run: &RunDoc) -> Self {
        Self {
            event_id: format!("{}-{}", run.run_id, run.updated_at_secs),
            work_id: run.work_id.clone(),
            event_kind: "run".to_owned(),
            summary: format!("run {} {}", run.run_id, run.status),
            actor_kind: None,
            actor_id: None,
            source: None,
            before_status: None,
            after_status: None,
            outcome: None,
            evidence_summary: None,
            happened_at_secs: run.updated_at_secs,
            priority: 1,
        }
    }

    fn from_run_failure(run: &RunDoc, reason: &str) -> Self {
        Self {
            event_id: format!("{}-failed-{}", run.run_id, run.updated_at_secs),
            work_id: run.work_id.clone(),
            event_kind: "run".to_owned(),
            summary: format!("run {} failed: {reason}", run.run_id),
            actor_kind: None,
            actor_id: None,
            source: Some("runtime".to_owned()),
            before_status: None,
            after_status: None,
            outcome: Some("failed".to_owned()),
            evidence_summary: None,
            happened_at_secs: run.updated_at_secs,
            priority: 1,
        }
    }

    fn from_run_completion(run: &RunDoc) -> Self {
        Self {
            event_id: format!("{}-completed-{}", run.run_id, run.updated_at_secs),
            work_id: run.work_id.clone(),
            event_kind: "run".to_owned(),
            summary: format!("run {} completed", run.run_id),
            actor_kind: None,
            actor_id: None,
            source: Some("runtime".to_owned()),
            before_status: None,
            after_status: None,
            outcome: Some("completed".to_owned()),
            evidence_summary: None,
            happened_at_secs: run.updated_at_secs,
            priority: 1,
        }
    }

    fn into_view(self) -> Result<ActivityEntryView, StoreError> {
        Ok(ActivityEntryView {
            event_kind: self.event_kind,
            work_id: self.work_id,
            summary: self.summary,
            actor_kind: self
                .actor_kind
                .as_deref()
                .map(parse_actor_kind)
                .transpose()?,
            actor_id: self.actor_id,
            source: self.source,
            before_status: self
                .before_status
                .as_deref()
                .map(parse_work_status)
                .transpose()?,
            after_status: self
                .after_status
                .as_deref()
                .map(parse_work_status)
                .transpose()?,
            outcome: self.outcome,
            evidence_summary: self.evidence_summary,
        })
    }
}

impl WorkCommentDoc {
    fn into_view(self) -> Result<WorkCommentView, StoreError> {
        Ok(WorkCommentView {
            author_kind: parse_actor_kind(&self.author_kind)?,
            author_id: self.author_id,
            source: self.source,
            body: self.body,
        })
    }
}

impl SurrealStore {
    fn append_work_comment(
        &self,
        company_id: &CompanyId,
        work_id: &WorkId,
        author_kind: ActorKind,
        author_id: &crate::model::ActorId,
        source: Option<String>,
        body: String,
    ) -> Result<String, StoreError> {
        let (comment_id, created_at_secs) = self.update_store_meta(|meta| {
            meta.next_comment_seq += 1;
            meta.tick += 1;
            (format!("comment-{}", meta.next_comment_seq), meta.tick)
        })?;
        let comment = WorkCommentDoc {
            comment_id: comment_id.clone(),
            company_id: company_id.to_string(),
            work_id: work_id.to_string(),
            author_kind: actor_kind_label(author_kind).to_owned(),
            author_id: author_id.to_string(),
            source,
            body,
            created_at_secs,
        };
        self.upsert_record(&work_comment_record_id(&comment_id), comment.clone())?;
        self.upsert_record(
            &activity_event_record_id(&comment.comment_id),
            ActivityEventDoc::from_comment(&comment),
        )?;
        Ok(comment_id)
    }

    fn persist_run_activity(&self, run: &RunDoc) -> Result<(), StoreError> {
        let event = ActivityEventDoc::from_run(run);
        self.upsert_record(&activity_event_record_id(&event.event_id), event)
    }

    fn persist_run_failure_activity(&self, run: &RunDoc, reason: &str) -> Result<(), StoreError> {
        let event = ActivityEventDoc::from_run_failure(run, reason);
        self.upsert_record(&activity_event_record_id(&event.event_id), event)
    }

    fn persist_run_completion_activity(&self, run: &RunDoc) -> Result<(), StoreError> {
        let event = ActivityEventDoc::from_run_completion(run);
        self.upsert_record(&activity_event_record_id(&event.event_id), event)
    }

    fn ensure_runnable_run(
        &self,
        snapshot: &WorkDoc,
        agent_id: &AgentId,
        updated_at_secs: u64,
    ) -> Result<Option<RunDoc>, StoreError> {
        let runs = self.query_docs_with_bind::<RunDoc, _>(
            "SELECT * FROM run WHERE work_id = $work_id ORDER BY updated_at_secs DESC",
            "work_id",
            snapshot.work_id.clone(),
        )?;
        if let Some(run) = runs.into_iter().find(|run| {
            run.work_id == snapshot.work_id
                && parse_run_status(&run.status).is_ok_and(|status| status.is_runnable())
        }) {
            let previous_status = run.status.clone();
            let next = RunDoc {
                agent_id: agent_id.to_string(),
                status: run_status_label(RunStatus::Running).to_owned(),
                updated_at_secs,
                ..run
            };
            self.upsert_record(&run_record_id(&next.run_id), next.clone())?;
            if previous_status != run_status_label(RunStatus::Running) {
                self.persist_run_activity(&next)?;
            }
            return Ok(Some(next));
        }

        let run_id = self.update_store_meta(|meta| {
            meta.next_run_seq += 1;
            format!("run-{}", meta.next_run_seq)
        })?;
        let run = RunDoc {
            run_id,
            company_id: snapshot.company_id.clone(),
            agent_id: agent_id.to_string(),
            work_id: snapshot.work_id.clone(),
            status: run_status_label(RunStatus::Running).to_owned(),
            created_at_secs: updated_at_secs,
            updated_at_secs,
        };
        self.upsert_record(&run_record_id(&run.run_id), run.clone())?;
        self.persist_run_activity(&run)?;
        Ok(Some(run))
    }

    fn ensure_wake_runnable_run(
        &self,
        snapshot: &WorkDoc,
        updated_at_secs: u64,
    ) -> Result<Option<RunDoc>, StoreError> {
        let has_open_lease = snapshot
            .active_lease_id
            .as_ref()
            .and_then(|lease_id| {
                self.select_record::<LeaseDoc>(&lease_record_id(lease_id))
                    .ok()
            })
            .flatten()
            .is_some_and(|lease| lease.released_at_secs.is_none());

        let runs = self.query_docs_with_bind::<RunDoc, _>(
            "SELECT * FROM run WHERE work_id = $work_id ORDER BY updated_at_secs DESC",
            "work_id",
            snapshot.work_id.clone(),
        )?;
        let existing_run = runs.into_iter().find(|run| {
            run.work_id == snapshot.work_id
                && parse_run_status(&run.status).is_ok_and(|status| status.is_runnable())
        });
        let runnable_agent = self.pick_runnable_agent(snapshot).ok();

        match kernel::wake_run_plan(
            has_open_lease,
            existing_run.is_some(),
            runnable_agent.is_some(),
        ) {
            crate::kernel::WakeRunPlan::SkipBecauseOpenLease
            | crate::kernel::WakeRunPlan::SkipBecauseNoRunnableAgent => Ok(None),
            crate::kernel::WakeRunPlan::RefreshExistingRun => {
                let run = existing_run.expect("existing run should exist");
                let next = RunDoc {
                    updated_at_secs,
                    ..run
                };
                self.upsert_record(&run_record_id(&next.run_id), next.clone())?;
                Ok(Some(next))
            }
            crate::kernel::WakeRunPlan::QueueNewRun => {
                let agent_id = runnable_agent
                    .ok_or_else(|| conflict("no runnable agent is available for wake queue"))?;
                let run_id = self.update_store_meta(|meta| {
                    meta.next_run_seq += 1;
                    format!("run-{}", meta.next_run_seq)
                })?;
                let run = RunDoc {
                    run_id,
                    company_id: snapshot.company_id.clone(),
                    agent_id,
                    work_id: snapshot.work_id.clone(),
                    status: run_status_label(RunStatus::Queued).to_owned(),
                    created_at_secs: updated_at_secs,
                    updated_at_secs,
                };
                self.upsert_record(&run_record_id(&run.run_id), run.clone())?;
                self.persist_run_activity(&run)?;
                Ok(Some(run))
            }
        }
    }

    fn find_open_lease_for_run(
        &self,
        work_id: &str,
        run_id: &str,
    ) -> Result<Option<crate::model::LeaseId>, StoreError> {
        let snapshot = self.select_record::<WorkDoc>(&work_record_id(work_id))?;
        let direct = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.active_lease_id.clone());
        let fallback = self
            .query_docs_with_bind::<LeaseDoc, _>(
                "SELECT * FROM lease WHERE run_id = $run_id ORDER BY acquired_at_secs DESC",
                "run_id",
                run_id.to_owned(),
            )?
            .into_iter()
            .find(|lease| {
                lease.work_id == work_id
                    && lease.released_at_secs.is_none()
                    && lease.run_id.as_deref() == Some(run_id)
            })
            .map(|lease| lease.lease_id);
        let lease_id = direct.or(fallback);

        Ok(lease_id.map(crate::model::LeaseId::from))
    }

    fn pick_runnable_agent(&self, snapshot: &WorkDoc) -> Result<String, StoreError> {
        if let Some(agent_id) = snapshot.assignee_agent_id.as_ref() {
            if let Some(agent) = self.select_record::<AgentDoc>(&agent_record_id(agent_id))? {
                if agent.company_id == snapshot.company_id
                    && parse_agent_status(&agent.status)? == AgentStatus::Active
                {
                    return Ok(agent_id.clone());
                }
            }
        }

        self.query_docs_with_binds::<AgentDoc, _, _>(
            "SELECT * FROM agent WHERE company_id = $company_id AND status = $status ORDER BY agent_id ASC",
            ("company_id", snapshot.company_id.clone()),
            ("status", agent_status_label(AgentStatus::Active).to_owned()),
        )?
            .into_iter()
            .find(|agent| agent.company_id == snapshot.company_id)
            .map(|agent| agent.agent_id)
            .ok_or_else(|| conflict("no runnable agent is available for wake queue"))
    }
}

fn contracts_for_company(
    store: &SurrealStore,
    company_id: &CompanyId,
) -> Result<Vec<ContractRevisionDoc>, StoreError> {
    store.query_docs_with_bind::<ContractRevisionDoc, _>(
        "SELECT * FROM contract_revision WHERE company_id = $company_id ORDER BY revision ASC",
        "company_id",
        company_id.to_string(),
    )
}

fn active_contract_for_company(
    store: &SurrealStore,
    company_id: &CompanyId,
) -> Result<Option<ContractSet>, StoreError> {
    let mut contracts = contracts_for_company(store, company_id)?;
    contracts.sort_by_key(|contract| contract.revision);
    contracts
        .into_iter()
        .find(|contract| contract.status == contract_status_label(ContractSetStatus::Active))
        .map(contract_doc_into_model)
        .transpose()
}

fn contract_doc_into_model(doc: ContractRevisionDoc) -> Result<ContractSet, StoreError> {
    Ok(ContractSet {
        contract_set_id: ContractSetId::from(doc.contract_set_id),
        company_id: CompanyId::from(doc.company_id),
        revision: doc.revision,
        name: doc.name,
        status: parse_contract_status(&doc.status)?,
        rules: serde_json::from_value(doc.rules_json)
            .map_err(|error| unavailable(&format!("contract rules decode failed: {error}")))?,
    })
}

fn contract_for_work(store: &SurrealStore, snapshot: &WorkDoc) -> Result<ContractSet, StoreError> {
    store
        .select_record::<ContractRevisionDoc>(&contract_record_id(
            &snapshot.contract_set_id,
            snapshot.contract_rev,
        ))?
        .ok_or_else(|| unavailable("work pinned contract revision is missing in surreal store"))
        .and_then(contract_doc_into_model)
}

fn company_record_id(company_id: &str) -> String {
    format!("company:{company_id}")
}

fn agent_record_id(agent_id: &str) -> String {
    format!("agent:{agent_id}")
}

fn contract_record_id(contract_set_id: &str, revision: u32) -> String {
    format!("contract_revision:{contract_set_id}:{revision}")
}

fn work_record_id(work_id: &str) -> String {
    format!("work:{work_id}")
}

fn lease_record_id(lease_id: &str) -> String {
    format!("lease:{lease_id}")
}

fn pending_wake_record_id(work_id: &str) -> String {
    format!("pending_wake:{work_id}")
}

fn run_record_id(run_id: &str) -> String {
    format!("run:{run_id}")
}

fn session_record_id(key: &SessionKey) -> String {
    session_record_id_parts(key.agent_id.as_str(), key.work_id.as_str())
}

fn session_record_id_parts(agent_id: &str, work_id: &str) -> String {
    format!("task_session:{agent_id}:{work_id}")
}

fn consumption_record_id(event_id: &str) -> String {
    format!("consumption_event:{event_id}")
}

fn work_comment_record_id(comment_id: &str) -> String {
    format!("work_comment:{comment_id}")
}

fn transition_record_id(record_id: &str) -> String {
    format!("transition_record:{record_id}")
}

fn activity_event_record_id(event_id: &str) -> String {
    format!("activity_event:{event_id}")
}

fn sequenced_id(sequence: u64) -> String {
    format!("00000000-0000-4000-8000-{sequence:012x}")
}

fn timestamp(tick: u64) -> Timestamp {
    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(tick)
}

fn timestamp_secs(timestamp: Timestamp) -> u64 {
    timestamp
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn write_snapshot_envelope(
    export: &StoreSnapshotEnvelope,
    export_path: &Path,
) -> Result<(), StoreError> {
    if let Some(parent) = export_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            unavailable(&format!(
                "surreal snapshot export could not create parent dir: {error}"
            ))
        })?;
    }

    let bytes = serde_json::to_vec_pretty(export).map_err(|error| {
        unavailable(&format!(
            "surreal snapshot export could not encode envelope: {error}"
        ))
    })?;
    fs::write(export_path, bytes).map_err(|error| {
        unavailable(&format!(
            "surreal snapshot export could not write file: {error}"
        ))
    })?;
    Ok(())
}

fn read_snapshot_envelope(export_path: &Path) -> Result<StoreSnapshotEnvelope, StoreError> {
    let bytes = fs::read(export_path).map_err(|error| {
        unavailable(&format!(
            "surreal snapshot import could not read export file: {error}"
        ))
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        unavailable(&format!(
            "surreal snapshot import could not decode export file: {error}"
        ))
    })
}

fn decode_snapshot_state(export: &StoreSnapshotEnvelope) -> Result<StoreSnapshotState, StoreError> {
    if export.format != SNAPSHOT_FORMAT {
        return Err(StoreError {
            kind: StoreErrorKind::Conflict,
            message: format!(
                "surreal snapshot import rejects unknown export format: {}",
                export.format
            ),
        });
    }

    let state_bytes = serde_json::to_vec(&export.state).map_err(|error| {
        unavailable(&format!(
            "surreal snapshot import could not encode export state: {error}"
        ))
    })?;
    let actual_checksum = fnv64_hex(&state_bytes);
    if actual_checksum != export.checksum_fnv64 {
        return Err(StoreError {
            kind: StoreErrorKind::Conflict,
            message: format!(
                "surreal snapshot checksum mismatch: expected {}, got {}",
                export.checksum_fnv64, actual_checksum
            ),
        });
    }

    let mut state = export.state.clone();
    state.normalize();
    Ok(state)
}

fn count_by_key<'a>(keys: impl Iterator<Item = &'a str>) -> BTreeMap<&'a str, usize> {
    let mut counts = BTreeMap::new();
    for key in keys {
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn work_comments_for(
    store: &SurrealStore,
    work_id: &str,
) -> Result<Vec<WorkCommentView>, StoreError> {
    store
        .query_docs_with_bind::<WorkCommentDoc, _>(
            "SELECT * FROM work_comment WHERE work_id = $work_id ORDER BY created_at_secs ASC",
            "work_id",
            work_id.to_owned(),
        )?
        .into_iter()
        .map(WorkCommentDoc::into_view)
        .collect()
}

fn activity_entries(store: &SurrealStore) -> Result<Vec<ActivityEntryView>, StoreError> {
    let entries = store
        .query_docs::<ActivityEventDoc>(
            "SELECT * FROM activity_event ORDER BY happened_at_secs DESC, priority DESC LIMIT 20",
        )?
        .into_iter()
        .map(|entry| {
            Ok(TimedActivityView {
                sort_key: entry.happened_at_secs,
                priority: entry.priority,
                view: entry.into_view()?,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(activity_views(entries.into_iter(), Some(20)))
}

fn work_activity_entries(
    store: &SurrealStore,
    work_id: &str,
) -> Result<Vec<ActivityEntryView>, StoreError> {
    let entries = store
        .query_docs_with_bind::<ActivityEventDoc, _>(
            "SELECT * FROM activity_event WHERE work_id = $work_id ORDER BY happened_at_secs DESC, priority DESC LIMIT 20",
            "work_id",
            work_id.to_owned(),
        )?
        .into_iter()
        .map(|entry| {
            Ok(TimedActivityView {
                sort_key: entry.happened_at_secs,
                priority: entry.priority,
                view: entry.into_view()?,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(activity_views(entries.into_iter(), Some(20)))
}

fn activity_views(
    entries: impl Iterator<Item = TimedActivityView>,
    limit: Option<usize>,
) -> Vec<ActivityEntryView> {
    let mut entries = entries.collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .sort_key
            .cmp(&left.sort_key)
            .then_with(|| right.priority.cmp(&left.priority))
    });
    let entries = entries.into_iter();
    match limit {
        Some(limit) => entries.take(limit).map(|entry| entry.view).collect(),
        None => entries.map(|entry| entry.view).collect(),
    }
}

fn consumption_summary(events: &[ConsumptionEventDoc]) -> ConsumptionSummaryView {
    let mut summary = ConsumptionSummaryView {
        total_turns: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_run_seconds: 0,
        total_estimated_cost_cents: 0,
    };
    for event in events {
        summary.total_turns += 1;
        summary.total_input_tokens += event.input_tokens;
        summary.total_output_tokens += event.output_tokens;
        summary.total_run_seconds += event.run_seconds;
        summary.total_estimated_cost_cents += event.estimated_cost_cents.unwrap_or(0);
    }
    summary
}

fn company_budget_hard_stopped_from_rollup((limit, spent): (Option<u64>, u64)) -> bool {
    limit.is_some_and(|limit| spent >= limit)
}

fn summarized_consumption(events: &[ConsumptionAgentDoc]) -> ConsumptionSummaryView {
    let mut summary = ConsumptionSummaryView {
        total_turns: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_run_seconds: 0,
        total_estimated_cost_cents: 0,
    };
    for event in events {
        summary.total_turns += 1;
        summary.total_input_tokens += event.input_tokens;
        summary.total_output_tokens += event.output_tokens;
        summary.total_run_seconds += event.run_seconds;
        summary.total_estimated_cost_cents += event.estimated_cost_cents.unwrap_or(0);
    }
    summary
}

fn grouped_work_comments(
    comments: Vec<WorkCommentDoc>,
) -> Result<BTreeMap<String, Vec<WorkCommentView>>, StoreError> {
    let mut comments_by_work = BTreeMap::new();
    for comment in comments {
        comments_by_work
            .entry(comment.work_id.clone())
            .or_insert_with(Vec::new)
            .push(comment.into_view()?);
    }
    Ok(comments_by_work)
}

fn grouped_work_activity_entries(
    entries: Vec<ActivityEventDoc>,
) -> Result<BTreeMap<String, Vec<ActivityEntryView>>, StoreError> {
    let entries = entries
        .into_iter()
        .map(|entry| {
            Ok((
                entry.work_id.clone(),
                TimedActivityView {
                    sort_key: entry.happened_at_secs,
                    priority: entry.priority,
                    view: entry.into_view()?,
                },
            ))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;

    let mut entries_by_work = BTreeMap::<String, Vec<TimedActivityView>>::new();
    for (work_id, entry) in entries {
        entries_by_work.entry(work_id).or_default().push(entry);
    }

    Ok(entries_by_work
        .into_iter()
        .map(|(work_id, entries)| (work_id, activity_views(entries.into_iter(), Some(20))))
        .collect())
}

fn agent_consumption_summaries(
    agents: &[AgentDoc],
    events: &[ConsumptionAgentDoc],
) -> Vec<AgentConsumptionSummaryView> {
    let mut totals = BTreeMap::<&str, AgentConsumptionSummaryView>::new();
    for event in events {
        let summary =
            totals
                .entry(event.agent_id.as_str())
                .or_insert_with(|| AgentConsumptionSummaryView {
                    agent_id: event.agent_id.clone(),
                    total_turns: 0,
                    total_input_tokens: 0,
                    total_output_tokens: 0,
                    total_run_seconds: 0,
                    total_estimated_cost_cents: 0,
                });
        summary.total_turns += 1;
        summary.total_input_tokens += event.input_tokens;
        summary.total_output_tokens += event.output_tokens;
        summary.total_run_seconds += event.run_seconds;
        summary.total_estimated_cost_cents += event.estimated_cost_cents.unwrap_or(0);
    }

    agents
        .iter()
        .map(|agent| {
            totals
                .get(agent.agent_id.as_str())
                .cloned()
                .unwrap_or_else(|| AgentConsumptionSummaryView {
                    agent_id: agent.agent_id.clone(),
                    total_turns: 0,
                    total_input_tokens: 0,
                    total_output_tokens: 0,
                    total_run_seconds: 0,
                    total_estimated_cost_cents: 0,
                })
        })
        .collect()
}

fn reaper_comment_body(
    run_id: &RunId,
    released_lease_id: Option<&crate::model::LeaseId>,
    follow_up_run_id: Option<&RunId>,
) -> String {
    let lease = released_lease_id
        .map(|lease_id| format!(" lease {} expired;", lease_id))
        .unwrap_or_default();
    let follow_up = follow_up_run_id
        .map(|queued_run_id| format!(" follow-up {} queued.", queued_run_id))
        .unwrap_or_else(|| " no follow-up queued.".to_owned());

    format!(
        "system reaper timed out run {}.{}{}",
        run_id, lease, follow_up
    )
}

fn wake_comment_body(
    source: &str,
    reason: &str,
    obligations: &[String],
    merged_count: u32,
) -> String {
    let obligations = if obligations.is_empty() {
        "none".to_owned()
    } else {
        obligations.join(", ")
    };

    format!(
        "{source} wake merged: reason={reason}; count={merged_count}; obligations={obligations}"
    )
}

fn default_json_object() -> serde_json::Value {
    serde_json::json!({})
}

fn default_json_array() -> serde_json::Value {
    serde_json::json!([])
}

fn work_kind_label(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Objective => "objective",
        WorkKind::Project => "project",
        WorkKind::Task => "task",
        WorkKind::Decision => "decision",
    }
}

fn parse_work_kind(kind: &str) -> Result<WorkKind, StoreError> {
    match kind {
        "objective" => Ok(WorkKind::Objective),
        "project" => Ok(WorkKind::Project),
        "task" => Ok(WorkKind::Task),
        "decision" => Ok(WorkKind::Decision),
        _ => Err(unavailable("unknown work kind in surreal document")),
    }
}

fn work_status_label(status: WorkStatus) -> &'static str {
    match status {
        WorkStatus::Backlog => "backlog",
        WorkStatus::Todo => "todo",
        WorkStatus::Doing => "doing",
        WorkStatus::Blocked => "blocked",
        WorkStatus::Done => "done",
        WorkStatus::Cancelled => "cancelled",
    }
}

fn parse_work_status(status: &str) -> Result<WorkStatus, StoreError> {
    match status {
        "backlog" => Ok(WorkStatus::Backlog),
        "todo" => Ok(WorkStatus::Todo),
        "doing" => Ok(WorkStatus::Doing),
        "blocked" => Ok(WorkStatus::Blocked),
        "done" => Ok(WorkStatus::Done),
        "cancelled" => Ok(WorkStatus::Cancelled),
        _ => Err(unavailable("unknown work status in surreal document")),
    }
}

fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::Critical => "critical",
        Priority::High => "high",
        Priority::Medium => "medium",
        Priority::Low => "low",
    }
}

fn parse_priority(priority: &str) -> Result<Priority, StoreError> {
    match priority {
        "critical" => Ok(Priority::Critical),
        "high" => Ok(Priority::High),
        "medium" => Ok(Priority::Medium),
        "low" => Ok(Priority::Low),
        _ => Err(unavailable("unknown priority in surreal document")),
    }
}

fn contract_status_label(status: ContractSetStatus) -> &'static str {
    match status {
        ContractSetStatus::Draft => "draft",
        ContractSetStatus::Active => "active",
        ContractSetStatus::Retired => "retired",
    }
}

fn parse_contract_status(status: &str) -> Result<ContractSetStatus, StoreError> {
    match status {
        "draft" => Ok(ContractSetStatus::Draft),
        "active" => Ok(ContractSetStatus::Active),
        "retired" => Ok(ContractSetStatus::Retired),
        _ => Err(unavailable("unknown contract status in surreal document")),
    }
}

fn agent_status_label(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Active => "active",
        AgentStatus::Paused => "paused",
        AgentStatus::Terminated => "terminated",
    }
}

fn parse_agent_status(status: &str) -> Result<AgentStatus, StoreError> {
    match status {
        "active" => Ok(AgentStatus::Active),
        "paused" => Ok(AgentStatus::Paused),
        "terminated" => Ok(AgentStatus::Terminated),
        _ => Err(unavailable("unknown agent status in surreal document")),
    }
}

fn actor_kind_label(kind: ActorKind) -> &'static str {
    match kind {
        ActorKind::Agent => "agent",
        ActorKind::Board => "board",
        ActorKind::System => "system",
    }
}

fn parse_actor_kind(kind: &str) -> Result<ActorKind, StoreError> {
    match kind {
        "agent" => Ok(ActorKind::Agent),
        "board" => Ok(ActorKind::Board),
        "system" => Ok(ActorKind::System),
        _ => Err(unavailable("unknown actor kind in surreal document")),
    }
}

fn decision_outcome_label(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
        DecisionOutcome::Conflict => "conflict",
        DecisionOutcome::OverrideAccepted => "override_accepted",
    }
}

fn transition_kind_label(kind: TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Queue => "queue",
        TransitionKind::Claim => "claim",
        TransitionKind::ProposeProgress => "propose_progress",
        TransitionKind::Complete => "complete",
        TransitionKind::Block => "block",
        TransitionKind::Reopen => "reopen",
        TransitionKind::Cancel => "cancel",
        TransitionKind::OverrideComplete => "override_complete",
        TransitionKind::TimeoutRequeue => "timeout_requeue",
    }
}

fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        RunStatus::TimedOut => "timed_out",
    }
}

fn parse_run_status(status: &str) -> Result<RunStatus, StoreError> {
    match status {
        "queued" => Ok(RunStatus::Queued),
        "running" => Ok(RunStatus::Running),
        "completed" => Ok(RunStatus::Completed),
        "failed" => Ok(RunStatus::Failed),
        "cancelled" => Ok(RunStatus::Cancelled),
        "timed_out" => Ok(RunStatus::TimedOut),
        _ => Err(unavailable("unknown run status in surreal document")),
    }
}

fn parse_decision_outcome(outcome: &str) -> Result<DecisionOutcome, StoreError> {
    match outcome {
        "accepted" => Ok(DecisionOutcome::Accepted),
        "rejected" => Ok(DecisionOutcome::Rejected),
        "conflict" => Ok(DecisionOutcome::Conflict),
        "override_accepted" => Ok(DecisionOutcome::OverrideAccepted),
        _ => Err(unavailable("unknown decision outcome in surreal document")),
    }
}

fn runtime_kind_label(runtime: RuntimeKind) -> &'static str {
    match runtime {
        RuntimeKind::Coclai => "coclai",
    }
}

fn parse_transition_kind(kind: &str) -> Result<TransitionKind, StoreError> {
    match kind {
        "queue" => Ok(TransitionKind::Queue),
        "claim" => Ok(TransitionKind::Claim),
        "propose_progress" => Ok(TransitionKind::ProposeProgress),
        "complete" => Ok(TransitionKind::Complete),
        "block" => Ok(TransitionKind::Block),
        "reopen" => Ok(TransitionKind::Reopen),
        "cancel" => Ok(TransitionKind::Cancel),
        "override_complete" => Ok(TransitionKind::OverrideComplete),
        "timeout_requeue" => Ok(TransitionKind::TimeoutRequeue),
        _ => Err(unavailable("unknown transition kind in surreal document")),
    }
}

fn parse_runtime_kind(runtime: &str) -> Result<RuntimeKind, StoreError> {
    match runtime {
        "coclai" => Ok(RuntimeKind::Coclai),
        _ => Err(unavailable("unknown runtime kind in surreal document")),
    }
}

fn billing_kind_label(kind: BillingKind) -> &'static str {
    match kind {
        BillingKind::Subscription => "subscription",
        BillingKind::Api => "api",
        BillingKind::Manual => "manual",
    }
}

fn lease_release_reason_label(reason: LeaseReleaseReason) -> &'static str {
    match reason {
        LeaseReleaseReason::Completed => "completed",
        LeaseReleaseReason::Blocked => "blocked",
        LeaseReleaseReason::Cancelled => "cancelled",
        LeaseReleaseReason::Overridden => "overridden",
        LeaseReleaseReason::Conflict => "conflict",
        LeaseReleaseReason::Expired => "expired",
    }
}

fn parse_lease_release_reason(reason: &str) -> Result<LeaseReleaseReason, StoreError> {
    match reason {
        "completed" => Ok(LeaseReleaseReason::Completed),
        "blocked" => Ok(LeaseReleaseReason::Blocked),
        "cancelled" => Ok(LeaseReleaseReason::Cancelled),
        "overridden" => Ok(LeaseReleaseReason::Overridden),
        "conflict" => Ok(LeaseReleaseReason::Conflict),
        "expired" => Ok(LeaseReleaseReason::Expired),
        _ => Err(unavailable(
            "unknown lease release reason in surreal document",
        )),
    }
}

fn release_reason_for(kind: TransitionKind) -> LeaseReleaseReason {
    match kind {
        TransitionKind::Complete => LeaseReleaseReason::Completed,
        TransitionKind::Block => LeaseReleaseReason::Blocked,
        TransitionKind::Cancel => LeaseReleaseReason::Cancelled,
        TransitionKind::OverrideComplete => LeaseReleaseReason::Overridden,
        TransitionKind::TimeoutRequeue => LeaseReleaseReason::Expired,
        TransitionKind::Queue
        | TransitionKind::Claim
        | TransitionKind::ProposeProgress
        | TransitionKind::Reopen => LeaseReleaseReason::Conflict,
    }
}

fn timeout_requeue_commit_req(
    snapshot: &WorkDoc,
    stale_run: &RunDoc,
    lease_id: &crate::model::LeaseId,
    reaped_at_secs: u64,
) -> CommitDecisionReq {
    let snapshot = snapshot
        .clone()
        .into_snapshot()
        .expect("snapshot document should convert");
    store_support::timeout_requeue_commit_req(
        &snapshot,
        &stale_run.run_id,
        lease_id,
        timestamp(reaped_at_secs),
    )
}

fn ensure_commit_preconditions(
    snapshot: &WorkDoc,
    live_lease: Option<&LeaseDoc>,
    req: &CommitDecisionReq,
) -> Result<(), StoreError> {
    if snapshot.rev != req.context.record.expected_rev {
        return Err(conflict(
            "commit_decision expected_rev does not match authoritative snapshot",
        ));
    }

    match req.effects.lease {
        LeaseEffect::Acquire => {
            if req.context.record.lease_id.is_none() {
                return Err(conflict(
                    "commit_decision acquire requires a lease_id on the record",
                ));
            }
            if snapshot.active_lease_id.is_some() {
                return Err(conflict(
                    "commit_decision acquire requires no active lease on the authoritative snapshot",
                ));
            }
        }
        LeaseEffect::Keep | LeaseEffect::Renew | LeaseEffect::Release | LeaseEffect::None => {
            let Some(lease_id) = req.context.record.lease_id.as_ref() else {
                return Ok(());
            };
            if live_lease.is_none() {
                return Err(conflict(
                    "commit_decision requires a live authoritative lease",
                ));
            }
            if snapshot.active_lease_id.as_deref() != Some(lease_id.as_str()) {
                return Err(conflict(
                    "commit_decision lease does not match authoritative snapshot",
                ));
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct PreparedClaimAcquire {
    lease: WorkLease,
    run: RunDoc,
    run_activity: Option<ActivityEventDoc>,
}

fn prepare_claim_acquire(
    store: &SurrealStore,
    snapshot: &WorkDoc,
    agent_id: &AgentId,
    lease_id: &LeaseId,
    acquired_at_secs: u64,
    capability: &str,
) -> Result<PreparedClaimAcquire, StoreError> {
    let agent = store
        .select_record::<AgentDoc>(&agent_record_id(agent_id.as_str()))?
        .ok_or_else(|| conflict(&format!("{capability} requires a registered agent")))?;

    if agent.company_id != snapshot.company_id {
        return Err(conflict(&format!(
            "{capability} rejects agent/work company boundary violations",
        )));
    }

    match parse_agent_status(&agent.status)? {
        AgentStatus::Active => {}
        AgentStatus::Paused => {
            return Err(conflict(&format!("{capability} rejects paused agents")))
        }
        AgentStatus::Terminated => {
            return Err(conflict(&format!("{capability} rejects terminated agents")));
        }
    }

    if parse_work_status(&snapshot.status)? != WorkStatus::Todo {
        return Err(conflict(&format!(
            "{capability} requires a todo snapshot without an open lease",
        )));
    }

    if snapshot.active_lease_id.is_some()
        || store
            .query_docs_with_bind::<LeaseDoc, _>(
                "SELECT * FROM lease WHERE work_id = $work_id ORDER BY acquired_at_secs DESC",
                "work_id",
                snapshot.work_id.clone(),
            )?
            .into_iter()
            .any(|lease| lease.released_at_secs.is_none())
    {
        return Err(conflict("work already has an open lease"));
    }

    let (run, run_activity) =
        prepare_runnable_run_for_claim(store, snapshot, agent_id, acquired_at_secs)?;
    let lease = WorkLease {
        lease_id: lease_id.clone(),
        company_id: CompanyId::from(snapshot.company_id.clone()),
        work_id: WorkId::from(snapshot.work_id.clone()),
        agent_id: agent_id.clone(),
        run_id: Some(RunId::from(run.run_id.clone())),
        acquired_at: timestamp(acquired_at_secs),
        expires_at: None,
        released_at: None,
        release_reason: None,
    };

    Ok(PreparedClaimAcquire {
        lease,
        run,
        run_activity,
    })
}

fn prepare_runnable_run_for_claim(
    store: &SurrealStore,
    snapshot: &WorkDoc,
    agent_id: &AgentId,
    updated_at_secs: u64,
) -> Result<(RunDoc, Option<ActivityEventDoc>), StoreError> {
    let runs = store.query_docs_with_bind::<RunDoc, _>(
        "SELECT * FROM run WHERE work_id = $work_id ORDER BY updated_at_secs DESC",
        "work_id",
        snapshot.work_id.clone(),
    )?;
    if let Some(run) = runs.into_iter().find(|run| {
        run.work_id == snapshot.work_id
            && parse_run_status(&run.status).is_ok_and(|status| status.is_runnable())
    }) {
        let previous_status = run.status.clone();
        let next = RunDoc {
            agent_id: agent_id.to_string(),
            status: run_status_label(RunStatus::Running).to_owned(),
            updated_at_secs,
            ..run
        };
        let run_activity = (previous_status != run_status_label(RunStatus::Running))
            .then(|| ActivityEventDoc::from_run(&next));
        return Ok((next, run_activity));
    }

    let run_id = store.update_store_meta(|meta| {
        meta.next_run_seq += 1;
        format!("run-{}", meta.next_run_seq)
    })?;
    let run = RunDoc {
        run_id,
        company_id: snapshot.company_id.clone(),
        agent_id: agent_id.to_string(),
        work_id: snapshot.work_id.clone(),
        status: run_status_label(RunStatus::Running).to_owned(),
        created_at_secs: updated_at_secs,
        updated_at_secs,
    };
    let run_activity = Some(ActivityEventDoc::from_run(&run));
    Ok((run, run_activity))
}

fn claim_actor_agent_id(
    record: &TransitionRecord,
    capability: &str,
) -> Result<AgentId, StoreError> {
    if record.actor_kind != ActorKind::Agent {
        return Err(conflict(&format!(
            "{capability} acquire requires an agent actor",
        )));
    }

    Ok(AgentId::from(record.actor_id.as_str()))
}

fn would_create_work_cycle(
    store: &SurrealStore,
    work_id: &str,
    parent_id: &str,
) -> Result<bool, StoreError> {
    let mut current = Some(parent_id.to_owned());
    while let Some(candidate) = current {
        if candidate == work_id {
            return Ok(true);
        }
        current = store
            .select_record::<WorkDoc>(&work_record_id(&candidate))?
            .and_then(|doc| doc.parent_id);
    }
    Ok(false)
}

fn conflict(message: &str) -> StoreError {
    StoreError {
        kind: StoreErrorKind::Conflict,
        message: message.to_owned(),
    }
}

fn not_found(capability: &str, entity_id: &str) -> StoreError {
    StoreError {
        kind: StoreErrorKind::NotFound,
        message: format!(
            "surreal store capability `{capability}` could not find entity {entity_id}"
        ),
    }
}

fn store_path(store_url: &str) -> Result<PathBuf, StoreError> {
    let path = store_url
        .strip_prefix("surrealkv://")
        .ok_or_else(|| unavailable("surreal store requires surrealkv:// store_url"))?;

    if path.trim().is_empty() {
        return Err(unavailable("surreal store path must be non-empty"));
    }

    Ok(PathBuf::from(path))
}

fn unavailable(message: &str) -> StoreError {
    StoreError {
        kind: StoreErrorKind::Unavailable,
        message: message.to_owned(),
    }
}

fn fnv64_hex(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;

    let mut hash = Fnv64(OFFSET);
    bytes.hash(&mut hash);
    format!("{:016x}", hash.0)
}

struct Fnv64(u64);

impl Hasher for Fnv64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        const PRIME: u64 = 0x100000001b3;
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        adapter::memory::store::{DEMO_COMPANY_ID, DEMO_DOING_WORK_ID},
        model::{
            workspace_fingerprint, ActorId, ActorKind, AgentId, AgentStatus, CompanyId,
            ContractSetId, ContractSetStatus, DecisionOutcome, EvidenceBundle, EvidenceInline,
            EvidenceRef, GateResult, GateSpec, LeaseEffect, LeaseId, PendingWakeEffect, RecordId,
            RunId, RuntimeKind, SessionId, TaskSession, TransitionDecision, TransitionKind,
            TransitionRecord, TransitionRule, WorkId, WorkKind, WorkPatch, WorkSnapshot,
            WorkStatus,
        },
        port::store::{
            ActivateContractReq, AppendCommentReq, ClaimLeaseReq, CommitDecisionReq,
            CreateAgentReq, CreateCompanyReq, CreateContractDraftReq, CreateWorkReq, MergeWakeReq,
            RecordConsumptionReq, SetAgentStatusReq, StoreErrorKind, StorePort, UpdateWorkReq,
        },
    };

    use super::{
        agent_record_id, contract_record_id, pending_wake_record_id, run_record_id, store_path,
        timestamp, transition_record_id, work_record_id, ActivityEventDoc, AgentDoc,
        ContractRevisionDoc, PendingWakeDoc, RunDoc, StoreMeta, SurrealStore, TransitionRecordDoc,
        WorkDoc, CORE_SCHEMA_SQL, DEFAULT_DATABASE, DEFAULT_NAMESPACE,
    };

    #[test]
    fn parses_surrealkv_store_url_into_path() {
        let path = store_path("surrealkv://.axiomnexus/state.db").expect("path should parse");

        assert_eq!(path.to_string_lossy(), ".axiomnexus/state.db");
    }

    #[test]
    fn rejects_non_surrealkv_store_url() {
        let error = store_path("file://localhost/axiomnexus").expect_err("scheme should fail");

        assert!(error.message.contains("surrealkv://"));
    }

    #[test]
    fn opens_embedded_database_with_default_namespace_and_database() {
        let unique = format!(
            "surrealkv://{}/axiomnexus-surreal-{}",
            std::env::temp_dir().display(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_nanos()
        );

        let store = SurrealStore::open(&unique).expect("surreal store should open");

        assert_eq!(DEFAULT_NAMESPACE, "axiomnexus");
        assert_eq!(DEFAULT_DATABASE, "primary");
        drop(store);
    }

    #[test]
    fn core_schema_query_mentions_authoritative_tables() {
        for table in [
            "DEFINE TABLE store_meta SCHEMALESS;",
            "DEFINE TABLE company SCHEMALESS;",
            "DEFINE TABLE agent SCHEMALESS;",
            "DEFINE TABLE contract_revision SCHEMALESS;",
            "DEFINE TABLE work SCHEMALESS;",
            "DEFINE TABLE lease SCHEMALESS;",
            "DEFINE TABLE pending_wake SCHEMALESS;",
            "DEFINE TABLE run SCHEMALESS;",
            "DEFINE TABLE task_session SCHEMALESS;",
            "DEFINE TABLE transition_record SCHEMALESS;",
            "DEFINE TABLE work_comment SCHEMALESS;",
            "DEFINE TABLE consumption_event SCHEMALESS;",
            "DEFINE TABLE activity_event SCHEMALESS;",
        ] {
            assert!(
                CORE_SCHEMA_SQL.contains(table),
                "missing schema for {table}"
            );
        }
    }

    #[test]
    fn snapshot_export_import_roundtrip_rehydrates_documents() {
        let source_url = new_store_url("snapshot-export");
        let export_path = snapshot_path("snapshot-export");
        let source = SurrealStore::open(&source_url).expect("surreal store should open");

        let export = source
            .export_snapshot_envelope()
            .expect("snapshot export should succeed");
        super::write_snapshot_envelope(&export, &export_path)
            .expect("snapshot export should write");
        drop(source);
        let import_url = new_store_url("snapshot-import");
        let imported =
            super::read_snapshot_envelope(&export_path).expect("snapshot export should read back");
        let imported_state =
            super::decode_snapshot_state(&imported).expect("snapshot export should decode");
        let imported_store = SurrealStore::open(&import_url).expect("imported store should open");
        imported_store
            .replace_from_snapshot(&imported_state)
            .expect("snapshot import should apply");

        assert_eq!(export.format, super::SNAPSHOT_FORMAT);
        assert_eq!(imported.checksum_fnv64, export.checksum_fnv64);
        assert!(imported_store
            .read_companies()
            .items
            .iter()
            .any(|company| company.company_id == DEMO_COMPANY_ID));
        assert_eq!(
            imported_store
                .read_work(Some(&WorkId::from(DEMO_DOING_WORK_ID)))
                .expect("work should load")
                .items[0]
                .status,
            WorkStatus::Doing
        );
    }

    #[test]
    fn snapshot_import_rejects_checksum_mismatch() {
        let source_url = new_store_url("snapshot-checksum");
        let export_path = snapshot_path("snapshot-checksum");
        let source = SurrealStore::open(&source_url).expect("surreal store should open");
        let mut export = source
            .export_snapshot_envelope()
            .expect("snapshot export should succeed");
        drop(source);
        export.checksum_fnv64 = "deadbeef".to_owned();
        super::write_snapshot_envelope(&export, &export_path)
            .expect("tampered export should write");

        let error = SurrealStore::import_snapshot_from_file(
            &new_store_url("snapshot-checksum-import"),
            &export_path,
        )
        .expect_err("checksum mismatch should fail");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("checksum mismatch"));
    }

    #[test]
    fn bootstrap_creates_default_store_meta() {
        let unique = format!(
            "surrealkv://{}/axiomnexus-surreal-meta-{}",
            std::env::temp_dir().display(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_nanos()
        );

        let store = SurrealStore::open(&unique).expect("surreal store should open");
        let meta = store.load_store_meta().expect("store_meta should load");

        assert_eq!(meta, StoreMeta::default());
    }

    #[test]
    fn create_company_and_agent_persist_profiles() {
        let store = new_store("company-agent");

        let company = store
            .create_company(CreateCompanyReq {
                name: "Acme".to_owned(),
                description: "release scope".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company should create");
        let agent = store
            .create_agent(CreateAgentReq {
                company_id: company.profile.company_id.clone(),
                name: "worker".to_owned(),
                role: "builder".to_owned(),
            })
            .expect("agent should create");

        let companies = store.read_companies();

        assert_eq!(agent.status, crate::model::AgentStatus::Active);
        assert!(companies.items.iter().any(|item| {
            item.company_id == company.profile.company_id.as_str()
                && item.agent_count == 1
                && item.work_count == 0
        }));
    }

    #[test]
    fn create_contract_activate_and_create_work_roundtrip() {
        let store = new_store("contract-work");
        let company = store
            .create_company(CreateCompanyReq {
                name: "Acme".to_owned(),
                description: "release scope".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company should create");

        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "axiomnexus-rust-next".to_owned(),
                rules: rules(),
            })
            .expect("draft should create");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("activation should succeed");

        let contracts = store.read_contracts();
        let work = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Decision,
                title: "Decide release".to_owned(),
                body: "Compare rollout options".to_owned(),
                contract_set_id: "00000000-0000-4000-8000-00000000dead".into(),
            })
            .expect_err("wrong contract set should fail");

        assert_eq!(work.kind, crate::port::store::StoreErrorKind::Conflict);

        let created = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Decision,
                title: "Decide release".to_owned(),
                body: "Compare rollout options".to_owned(),
                contract_set_id: contracts.contract_set_id.as_str().into(),
            })
            .expect("work should create");

        let read = store
            .read_work(Some(&created.snapshot.work_id))
            .expect("work should read");

        assert_eq!(contracts.status, ContractSetStatus::Active);
        assert_eq!(created.snapshot.status, WorkStatus::Backlog);
        assert_eq!(read.items[0].title, "Decide release");
        assert_eq!(read.items[0].kind, WorkKind::Decision);
    }

    #[test]
    fn update_work_rejects_tree_cycles() {
        let store = new_store("work-cycle");
        let company = store
            .create_company(CreateCompanyReq {
                name: "Acme".to_owned(),
                description: "release scope".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company should create");
        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "axiomnexus-rust-next".to_owned(),
                rules: rules(),
            })
            .expect("draft should create");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("activation should succeed");
        let contracts = store.read_contracts();

        let parent = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Project,
                title: "Parent".to_owned(),
                body: "parent".to_owned(),
                contract_set_id: contracts.contract_set_id.as_str().into(),
            })
            .expect("parent should create");
        let child = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: Some(parent.snapshot.work_id.clone()),
                kind: WorkKind::Task,
                title: "Child".to_owned(),
                body: "child".to_owned(),
                contract_set_id: contracts.contract_set_id.as_str().into(),
            })
            .expect("child should create");

        let error = store
            .update_work(UpdateWorkReq {
                work_id: parent.snapshot.work_id.clone(),
                parent_id: Some(child.snapshot.work_id.clone()),
                title: "Parent".to_owned(),
                body: "parent".to_owned(),
            })
            .expect_err("cycle should fail");

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Conflict);
        assert!(error.message.contains("tree cycles"));
    }

    #[test]
    fn save_and_load_session_roundtrip() {
        let store = new_store("session-roundtrip");
        let session = TaskSession {
            session_id: SessionId::from("session-1"),
            company_id: CompanyId::from("company-1"),
            agent_id: AgentId::from("agent-1"),
            work_id: WorkId::from("work-1"),
            runtime: RuntimeKind::Coclai,
            runtime_session_id: "runtime-1".to_owned(),
            cwd: "/repo".to_owned(),
            workspace_fingerprint: workspace_fingerprint("/repo"),
            contract_rev: 1,
            last_record_id: None,
            last_decision_summary: Some("accepted".to_owned()),
            last_gate_summary: None,
            updated_at: timestamp(20),
        };

        store.save_session(&session).expect("session should save");
        let loaded = store
            .load_session(&crate::port::store::SessionKey {
                agent_id: AgentId::from("agent-1"),
                work_id: WorkId::from("work-1"),
            })
            .expect("load should work")
            .expect("session should exist");

        assert_eq!(loaded, session);
    }

    #[test]
    fn read_run_returns_status_and_current_session() {
        let store = new_store("read-run");
        store
            .upsert_record(
                &run_record_id("run-1"),
                RunDoc {
                    run_id: "run-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    agent_id: "agent-1".to_owned(),
                    work_id: "work-1".to_owned(),
                    status: "running".to_owned(),
                    created_at_secs: 10,
                    updated_at_secs: 20,
                },
            )
            .expect("run should seed");
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-1"),
                company_id: CompanyId::from("company-1"),
                agent_id: AgentId::from("agent-1"),
                work_id: WorkId::from("work-1"),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-1".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: Some("running".to_owned()),
                last_gate_summary: None,
                updated_at: timestamp(20),
            })
            .expect("session should save");

        let run = store
            .read_run(&RunId::from("run-1"))
            .expect("run should load");

        assert_eq!(run.status, "running");
        assert_eq!(
            run.current_session
                .expect("session should be attached")
                .runtime_session_id,
            "runtime-1"
        );
    }

    #[test]
    fn set_agent_status_rejects_resuming_terminated_agent() {
        let store = new_store("agent-status");
        store
            .upsert_record(
                &agent_record_id("agent-1"),
                AgentDoc {
                    agent_id: "agent-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    name: "agent".to_owned(),
                    role: "builder".to_owned(),
                    status: "terminated".to_owned(),
                },
            )
            .expect("agent should seed");

        let error = store
            .set_agent_status(SetAgentStatusReq {
                agent_id: AgentId::from("agent-1"),
                status: AgentStatus::Active,
            })
            .expect_err("terminated agent cannot resume");

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Conflict);
        assert!(error.message.contains("terminated"));
    }

    #[test]
    fn load_next_queued_run_skips_paused_agents() {
        let store = new_store("queued-run");
        store
            .upsert_record(
                &agent_record_id("agent-paused"),
                AgentDoc {
                    agent_id: "agent-paused".to_owned(),
                    company_id: "company-1".to_owned(),
                    name: "paused".to_owned(),
                    role: "builder".to_owned(),
                    status: "paused".to_owned(),
                },
            )
            .expect("paused agent should seed");
        store
            .upsert_record(
                &agent_record_id("agent-active"),
                AgentDoc {
                    agent_id: "agent-active".to_owned(),
                    company_id: "company-1".to_owned(),
                    name: "active".to_owned(),
                    role: "builder".to_owned(),
                    status: "active".to_owned(),
                },
            )
            .expect("active agent should seed");
        store
            .upsert_record(
                &run_record_id("run-paused"),
                RunDoc {
                    run_id: "run-paused".to_owned(),
                    company_id: "company-1".to_owned(),
                    agent_id: "agent-paused".to_owned(),
                    work_id: "work-1".to_owned(),
                    status: "queued".to_owned(),
                    created_at_secs: 10,
                    updated_at_secs: 10,
                },
            )
            .expect("paused run should seed");
        store
            .upsert_record(
                &run_record_id("run-active"),
                RunDoc {
                    run_id: "run-active".to_owned(),
                    company_id: "company-1".to_owned(),
                    agent_id: "agent-active".to_owned(),
                    work_id: "work-2".to_owned(),
                    status: "queued".to_owned(),
                    created_at_secs: 11,
                    updated_at_secs: 11,
                },
            )
            .expect("active run should seed");

        let next = crate::app::cmd::run_scheduler::next_queued_run_id(
            &store
                .load_queued_runs()
                .expect("next queued run should load"),
        )
        .expect("active queued run should exist");

        assert_eq!(next, RunId::from("run-active"));
    }

    #[test]
    fn load_context_reads_pending_wake_and_pinned_contract() {
        let store = new_store("load-context");
        let company = store
            .create_company(CreateCompanyReq {
                name: "Acme".to_owned(),
                description: "release scope".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company should create");
        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "axiomnexus-rust-next".to_owned(),
                rules: rules(),
            })
            .expect("draft should create");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("activation should succeed");
        let contracts = store.read_contracts();
        let work = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Context".to_owned(),
                body: "body".to_owned(),
                contract_set_id: contracts.contract_set_id.as_str().into(),
            })
            .expect("work should create");
        store
            .upsert_record(
                &pending_wake_record_id(work.snapshot.work_id.as_str()),
                PendingWakeDoc {
                    work_id: work.snapshot.work_id.to_string(),
                    obligations: vec!["cargo test".to_owned()],
                    count: 1,
                    latest_reason: "gate failed".to_owned(),
                    merged_at_secs: 21,
                },
            )
            .expect("pending wake should seed");

        let context = store
            .load_context(&work.snapshot.work_id)
            .expect("context should load");

        assert_eq!(
            context.contract.contract_set_id.as_str(),
            contracts.contract_set_id
        );
        assert_eq!(
            context
                .pending_wake
                .expect("pending wake should exist")
                .latest_reason,
            "gate failed"
        );
    }

    #[test]
    fn load_queued_runs_marks_company_budget_hard_stop_candidates() {
        let store = new_store("queued-budget");
        let company = store
            .create_company(CreateCompanyReq {
                name: "Budget Co".to_owned(),
                description: "company hard stop".to_owned(),
                runtime_hard_stop_cents: Some(5),
            })
            .expect("company should create");
        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "budget-contract".to_owned(),
                rules: rules(),
            })
            .expect("draft should create");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("activation should succeed");
        let contract_set_id = store
            .read_companies()
            .items
            .into_iter()
            .find(|item| item.company_id == company.profile.company_id.as_str())
            .and_then(|item| item.active_contract_set_id)
            .expect("company should expose active contract");
        let agent = store
            .create_agent(CreateAgentReq {
                company_id: company.profile.company_id.clone(),
                name: "worker".to_owned(),
                role: "builder".to_owned(),
            })
            .expect("agent should create");
        let work = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Blocked work".to_owned(),
                body: "budget hard stop".to_owned(),
                contract_set_id: ContractSetId::from(contract_set_id),
            })
            .expect("work should create");
        store
            .merge_wake(MergeWakeReq {
                work_id: work.snapshot.work_id.clone(),
                actor_kind: ActorKind::Board,
                actor_id: ActorId::from("board"),
                source: "manual".to_owned(),
                reason: "budget".to_owned(),
                obligations: vec!["stay queued".to_owned()],
            })
            .expect("wake should queue a run");

        let queued_run = store
            .load_queued_runs()
            .expect("queued runs should load")
            .into_iter()
            .find(|candidate| candidate.agent_status == Some(AgentStatus::Active))
            .expect("queued run should exist");
        store
            .record_consumption(RecordConsumptionReq {
                company_id: company.profile.company_id.clone(),
                agent_id: agent.agent_id.clone(),
                run_id: queued_run.run_id.clone(),
                billing_kind: crate::model::BillingKind::Api,
                usage: crate::model::ConsumptionUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    run_seconds: 1,
                    estimated_cost_cents: Some(5),
                },
            })
            .expect("consumption should persist");

        let blocked_run = store
            .load_queued_runs()
            .expect("queued runs should load")
            .into_iter()
            .find(|candidate| candidate.run_id == queued_run.run_id)
            .expect("queued run should remain listed");

        assert!(blocked_run.budget_blocked);
    }

    #[test]
    fn load_runtime_turn_requires_active_agent_and_returns_contract() {
        let store = new_store("runtime-turn");
        store
            .upsert_record(
                &agent_record_id("agent-1"),
                AgentDoc {
                    agent_id: "agent-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    name: "agent".to_owned(),
                    role: "builder".to_owned(),
                    status: "active".to_owned(),
                },
            )
            .expect("agent should seed");
        store
            .upsert_record(
                &contract_record_id("contract-1", 1),
                ContractRevisionDoc {
                    contract_set_id: "contract-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    revision: 1,
                    name: "contract".to_owned(),
                    status: "active".to_owned(),
                    rules_json: serde_json::to_value(rules()).expect("rules should encode"),
                },
            )
            .expect("contract should seed");
        store
            .upsert_record(
                &work_record_id("work-1"),
                WorkDoc {
                    work_id: "work-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    parent_id: None,
                    kind: "task".to_owned(),
                    title: "work".to_owned(),
                    body: "body".to_owned(),
                    status: "todo".to_owned(),
                    priority: "medium".to_owned(),
                    assignee_agent_id: Some("agent-1".to_owned()),
                    active_lease_id: None,
                    rev: 0,
                    contract_set_id: "contract-1".to_owned(),
                    contract_rev: 1,
                    created_at_secs: 10,
                    updated_at_secs: 10,
                },
            )
            .expect("work should seed");
        store
            .upsert_record(
                &run_record_id("run-1"),
                RunDoc {
                    run_id: "run-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    agent_id: "agent-1".to_owned(),
                    work_id: "work-1".to_owned(),
                    status: "queued".to_owned(),
                    created_at_secs: 10,
                    updated_at_secs: 10,
                },
            )
            .expect("run should seed");

        let turn = store
            .load_runtime_turn(&RunId::from("run-1"))
            .expect("runtime turn should load");

        assert_eq!(turn.run_id, RunId::from("run-1"));
        assert_eq!(turn.contract.contract_set_id.as_str(), "contract-1");
    }

    #[test]
    fn record_consumption_rejects_boundary_violations() {
        let store = new_store("consumption");
        store
            .upsert_record(
                &run_record_id("run-1"),
                RunDoc {
                    run_id: "run-1".to_owned(),
                    company_id: "company-1".to_owned(),
                    agent_id: "agent-1".to_owned(),
                    work_id: "work-1".to_owned(),
                    status: "running".to_owned(),
                    created_at_secs: 10,
                    updated_at_secs: 10,
                },
            )
            .expect("run should seed");

        let error = store
            .record_consumption(crate::port::store::RecordConsumptionReq {
                company_id: CompanyId::from("company-2"),
                agent_id: AgentId::from("agent-1"),
                run_id: RunId::from("run-1"),
                billing_kind: crate::model::BillingKind::Api,
                usage: crate::model::ConsumptionUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    run_seconds: 1,
                    estimated_cost_cents: Some(1),
                },
            })
            .expect_err("boundary violation should fail");

        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Conflict);
    }

    #[test]
    fn claim_lease_acquires_single_open_lease_and_conflicts_on_second_claim() {
        let store = new_store("claim-lease");
        let seeded = seed_runtime_work(&store);

        let first = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-claim-1"),
            })
            .expect("first claim should succeed");
        let persisted = store
            .read_work(Some(&seeded.work_id))
            .expect("work should remain readable");

        let error = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.second_agent_id.clone(),
                lease_id: LeaseId::from("lease-claim-2"),
            })
            .expect_err("second open claim should conflict");
        let claim_record = store
            .load_transition_records(&seeded.work_id)
            .expect("claim records should load")
            .into_iter()
            .find(|record| record.kind == TransitionKind::Claim)
            .expect("claim transition should persist");

        assert_eq!(first.lease.lease_id, LeaseId::from("lease-claim-1"));
        assert_eq!(persisted.items[0].status, WorkStatus::Doing);
        assert_eq!(
            persisted.items[0].active_lease_id.as_deref(),
            Some("lease-claim-1")
        );
        assert!(first.lease.run_id.is_some());
        assert_eq!(error.kind, crate::port::store::StoreErrorKind::Conflict);
        assert_eq!(claim_record.expected_rev, 0);
        assert_eq!(claim_record.after_status, Some(WorkStatus::Doing));
    }

    #[test]
    fn merge_wake_creates_single_queued_run_when_work_has_no_open_lease() {
        let store = new_store("wake-queue");
        let seeded = seed_runtime_work(&store);

        let merged = store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "new requirement",
                &["cargo test"],
            ))
            .expect("wake merge should succeed");

        let runs = store
            .select_table::<RunDoc>("run")
            .expect("run docs should load");
        let runnable = runs
            .into_iter()
            .filter(|run| {
                run.work_id == seeded.work_id.as_str()
                    && run.status == "queued"
                    && run.agent_id == seeded.agent_id.as_str()
            })
            .collect::<Vec<_>>();

        assert_eq!(merged.count, 1);
        assert_eq!(runnable.len(), 1);
    }

    #[test]
    fn merge_wake_persists_coalesced_pending_wake() {
        let store = new_store("wake-coalesce");
        let seeded = seed_runtime_work(&store);

        let merged = store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "gate failed",
                &["run tests", "run fmt"],
            ))
            .expect("first wake merge should succeed");
        let merged_again = store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "another gate failed",
                &["run fmt"],
            ))
            .expect("second wake merge should succeed");
        let pending_wake = store
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(seeded.work_id.as_str()))
            .expect("pending wake lookup should work")
            .expect("pending wake should persist");

        assert_eq!(merged.count, 1);
        assert_eq!(merged_again.count, 2);
        assert_eq!(pending_wake.count, 2);
        assert_eq!(
            pending_wake.obligations,
            vec!["run fmt".to_owned(), "run tests".to_owned()]
        );
    }

    #[test]
    fn merge_wake_reuses_existing_runnable_run_without_queue_fanout() {
        let store = new_store("wake-no-fanout");
        let seeded = seed_runtime_work(&store);

        store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "first wake",
                &["cargo test"],
            ))
            .expect("first wake should succeed");
        store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "second wake",
                &["cargo fmt"],
            ))
            .expect("second wake should succeed");

        let runs = store
            .select_table::<RunDoc>("run")
            .expect("run docs should load");
        let runnable = runs
            .into_iter()
            .filter(|run| {
                run.work_id == seeded.work_id.as_str()
                    && run.status == "queued"
                    && run.agent_id == seeded.agent_id.as_str()
            })
            .collect::<Vec<_>>();
        let pending_wake = store
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(seeded.work_id.as_str()))
            .expect("pending wake lookup should work")
            .expect("pending wake should persist");

        assert_eq!(runnable.len(), 1);
        assert_eq!(pending_wake.count, 2);
        assert_eq!(
            pending_wake.obligations,
            vec!["cargo fmt".to_owned(), "cargo test".to_owned()]
        );
    }

    #[test]
    fn merge_wake_for_paused_agent_keeps_pending_wake_without_creating_run() {
        let store = new_store("wake-paused-agent");
        let seeded = seed_runtime_work(&store);
        store
            .set_agent_status(SetAgentStatusReq {
                agent_id: seeded.agent_id.clone(),
                status: AgentStatus::Paused,
            })
            .expect("pause should succeed");

        let merged = store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "paused agent wake",
                &["cargo test"],
            ))
            .expect("wake merge should succeed");
        let runs = store
            .select_table::<RunDoc>("run")
            .expect("run docs should load");
        let runnable = runs
            .into_iter()
            .filter(|run| {
                run.work_id == seeded.work_id.as_str()
                    && run.status == "queued"
                    && run.agent_id == seeded.agent_id.as_str()
            })
            .collect::<Vec<_>>();
        let pending_wake = store
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(seeded.work_id.as_str()))
            .expect("pending wake lookup should work");

        assert_eq!(merged.count, 1);
        assert!(pending_wake.is_some());
        assert!(runnable.is_empty());
    }

    #[test]
    fn reap_timed_out_running_run_releases_expired_lease_and_queues_follow_up() {
        let store = new_store("reaper");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-reaper"),
            })
            .expect("claim should create a running run");
        store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "retry timed out run",
                &["retry"],
            ))
            .expect("wake merge should persist pending wake");

        let reaped = store
            .reap_timed_out_runs(Duration::from_secs(1))
            .expect("reaper should succeed");
        let reaped_item = reaped
            .iter()
            .find(|item| {
                item.run_id
                    == claimed
                        .lease
                        .run_id
                        .clone()
                        .expect("claim run should exist")
            })
            .expect("claimed run should be reaped");
        let failed_run = store
            .select_record::<RunDoc>(&run_record_id(
                claimed
                    .lease
                    .run_id
                    .as_ref()
                    .expect("claimed lease should carry run")
                    .as_str(),
            ))
            .expect("failed run lookup should work")
            .expect("failed run should persist");
        let follow_up = store
            .select_record::<RunDoc>(&run_record_id(
                reaped_item
                    .follow_up_run_id
                    .as_ref()
                    .expect("follow up should queue")
                    .as_str(),
            ))
            .expect("follow-up lookup should work")
            .expect("follow-up should persist");
        let lease = store
            .select_record::<super::LeaseDoc>(&super::lease_record_id(
                claimed.lease.lease_id.as_str(),
            ))
            .expect("lease lookup should work")
            .expect("lease should persist");
        let timeout_record = store
            .load_transition_records(&seeded.work_id)
            .expect("transition records should load")
            .into_iter()
            .find(|record| record.kind == TransitionKind::TimeoutRequeue)
            .expect("timeout record should persist");
        let snapshot = store
            .select_record::<WorkDoc>(&work_record_id(seeded.work_id.as_str()))
            .expect("snapshot lookup should work")
            .expect("snapshot should persist")
            .into_snapshot()
            .expect("snapshot should decode");

        assert!(reaped.iter().any(|item| item.work_id == seeded.work_id));
        assert_eq!(reaped_item.released_lease_id, Some(claimed.lease.lease_id));
        assert_eq!(failed_run.status, "timed_out");
        assert_eq!(follow_up.status, "queued");
        assert_eq!(lease.release_reason.as_deref(), Some("expired"));
        assert!(lease.released_at_secs.is_some());
        assert_eq!(timeout_record.actor_kind, ActorKind::System);
        assert_eq!(timeout_record.actor_id, ActorId::from("system"));
        assert_eq!(timeout_record.before_status, WorkStatus::Doing);
        assert_eq!(timeout_record.after_status, Some(WorkStatus::Todo));
        assert_eq!(snapshot.status, WorkStatus::Todo);
        assert!(snapshot.active_lease_id.is_none());
    }

    #[test]
    fn commit_decision_updates_snapshot_record_session_and_pending_wake() {
        let store = new_store("commit-decision");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-block"),
            })
            .expect("claim should succeed");
        promote_claimed_work_to_doing(
            &store,
            &seeded.work_id,
            &seeded.agent_id,
            &claimed.lease.lease_id,
        );
        store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "needs verification",
                &["cargo test"],
            ))
            .expect("wake should seed");

        let context = store
            .load_context(&seeded.work_id)
            .expect("context should load");
        let decision = TransitionDecision {
            outcome: DecisionOutcome::Accepted,
            reasons: Vec::new(),
            next_snapshot: Some(WorkSnapshot {
                status: WorkStatus::Blocked,
                rev: context.snapshot.rev + 1,
                active_lease_id: None,
                updated_at: timestamp(42),
                ..context.snapshot.clone()
            }),
            lease_effect: LeaseEffect::Release,
            pending_wake_effect: PendingWakeEffect::Clear,
            gate_results: vec![GateResult {
                gate: GateSpec::ManualNotePresent,
                passed: true,
                detail: "note present".to_owned(),
            }],
            evidence: Default::default(),
            summary: "Block Accepted with next status Blocked".to_owned(),
        };
        let session = TaskSession {
            session_id: SessionId::from("session-1"),
            company_id: seeded.company_id.clone(),
            agent_id: seeded.agent_id.clone(),
            work_id: seeded.work_id.clone(),
            runtime: RuntimeKind::Coclai,
            runtime_session_id: "runtime-1".to_owned(),
            cwd: "/repo".to_owned(),
            workspace_fingerprint: workspace_fingerprint("/repo"),
            contract_rev: context.contract.revision,
            last_record_id: Some(RecordId::from("record-1")),
            last_decision_summary: Some(decision.summary.clone()),
            last_gate_summary: None,
            updated_at: timestamp(0),
        };
        let record = TransitionRecord {
            record_id: RecordId::from("record-1"),
            company_id: seeded.company_id.clone(),
            work_id: seeded.work_id.clone(),
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::from(seeded.agent_id.as_str()),
            run_id: claimed.lease.run_id.clone(),
            session_id: Some(session.session_id.clone()),
            lease_id: Some(claimed.lease.lease_id.clone()),
            expected_rev: context.snapshot.rev,
            contract_set_id: context.snapshot.contract_set_id.clone(),
            contract_rev: context.snapshot.contract_rev,
            before_status: context.snapshot.status,
            after_status: decision
                .next_snapshot
                .as_ref()
                .map(|snapshot| snapshot.status),
            outcome: DecisionOutcome::Accepted,
            reasons: decision.reasons.clone(),
            kind: TransitionKind::Block,
            patch: WorkPatch {
                summary: "blocked".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: vec!["needs reviewer".to_owned()],
            },
            gate_results: decision.gate_results.clone(),
            evidence: decision.evidence.clone(),
            evidence_inline: Some(EvidenceInline {
                summary: decision.summary.clone(),
            }),
            evidence_refs: Vec::<EvidenceRef>::new(),
            happened_at: timestamp(0),
        };

        let result = store
            .commit_decision(CommitDecisionReq::new(
                decision.clone(),
                record.clone(),
                Some(session.clone()),
            ))
            .expect("commit_decision should succeed");
        let persisted_record = store
            .select_record::<TransitionRecordDoc>(&transition_record_id("record-1"))
            .expect("transition record lookup should work")
            .expect("transition record should persist");
        let persisted_activity = store
            .select_record::<ActivityEventDoc>(&super::activity_event_record_id("record-1"))
            .expect("activity lookup should work")
            .expect("activity event should persist");
        let persisted_work = store
            .select_record::<WorkDoc>(&work_record_id(seeded.work_id.as_str()))
            .expect("work lookup should work")
            .expect("work should persist");

        assert_eq!(
            result
                .snapshot
                .as_ref()
                .expect("snapshot should persist")
                .status,
            WorkStatus::Blocked
        );
        assert!(result.pending_wake.is_none());
        assert!(result.lease.is_none());
        assert_ne!(
            result
                .session
                .as_ref()
                .expect("session should persist")
                .updated_at,
            session.updated_at
        );
        assert_eq!(
            result
                .activity_event
                .expect("activity event should exist")
                .summary,
            decision.summary
        );
        assert_eq!(persisted_record.outcome, "accepted");
        assert_eq!(persisted_record.reasons_json, serde_json::json!([]));
        assert_eq!(persisted_record.kind, "block");
        assert_eq!(
            persisted_record.run_id.as_deref(),
            claimed.lease.run_id.as_ref().map(|run_id| run_id.as_str())
        );
        assert_eq!(persisted_record.session_id.as_deref(), Some("session-1"));
        assert_eq!(persisted_activity.event_kind, "transition");
        assert_eq!(
            persisted_activity.summary,
            "Block Accepted with next status Blocked"
        );
        assert_ne!(persisted_record.happened_at_secs, 0);
        assert_eq!(
            persisted_work.updated_at_secs,
            persisted_record.happened_at_secs
        );
    }

    #[test]
    fn surreal_commit_transaction_appends_record_before_projection_updates() {
        let src = include_str!("store/commit.rs");
        let transition_upsert = src
            .find(
                "UPSERT type::record('transition_record', $transition_id) CONTENT $transition_doc;",
            )
            .expect("transition upsert should exist");
        let activity_upsert = src
            .find("UPSERT type::record('activity_event', $activity_id) CONTENT $activity_doc;")
            .expect("activity upsert should exist");
        let work_upsert = src
            .find("UPSERT type::record('work', $work_id) CONTENT $work_doc;")
            .expect("work upsert should exist");
        let lease_upsert = src
            .find("UPSERT type::record('lease', $lease_id) CONTENT $lease_doc;")
            .expect("lease upsert should exist");
        let session_upsert = src
            .find("UPSERT type::record('task_session', $session_key) CONTENT $session_doc;")
            .expect("session upsert should exist");

        assert!(transition_upsert < work_upsert);
        assert!(activity_upsert < work_upsert);
        assert!(transition_upsert < lease_upsert);
        assert!(activity_upsert < lease_upsert);
        assert!(transition_upsert < session_upsert);
        assert!(activity_upsert < session_upsert);
    }

    #[test]
    fn surreal_direct_claim_and_commit_acquire_share_same_helper() {
        let store_src = include_str!("store.rs");
        let commit_src = include_str!("store/commit.rs");
        assert!(store_src.contains("let prepared = prepare_claim_acquire("));
        assert!(commit_src.contains("Some(prepare_claim_acquire("));
    }

    #[test]
    fn commit_decision_rejects_stale_expected_rev() {
        let store = new_store("commit-stale-rev");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-stale-rev"),
            })
            .expect("claim should succeed");

        let error = store
            .commit_decision(CommitDecisionReq::new(
                TransitionDecision {
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::RevConflict],
                    next_snapshot: None,
                    lease_effect: LeaseEffect::None,
                    pending_wake_effect: PendingWakeEffect::Retain,
                    gate_results: Vec::new(),
                    evidence: Default::default(),
                    summary: "stale rev".to_owned(),
                },
                TransitionRecord {
                    record_id: RecordId::from("record-stale-rev"),
                    company_id: seeded.company_id.clone(),
                    work_id: seeded.work_id.clone(),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(seeded.agent_id.as_str()),
                    run_id: claimed.lease.run_id.clone(),
                    session_id: None,
                    lease_id: Some(claimed.lease.lease_id),
                    expected_rev: 999,
                    contract_set_id: ContractSetId::from(seeded.contract_set_id.as_str()),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::RevConflict],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "stale rev".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: Vec::new(),
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "stale rev".to_owned(),
                    }),
                    evidence_refs: Vec::new(),
                    happened_at: timestamp(120),
                },
                None,
            ))
            .expect_err("stale expected_rev should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("expected_rev"));
        assert!(store
            .select_record::<TransitionRecordDoc>(&transition_record_id("record-stale-rev"))
            .expect("transition record lookup should work")
            .is_none());
    }

    #[test]
    fn commit_decision_rejects_stale_live_lease() {
        let store = new_store("commit-stale-lease");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-stale-live"),
            })
            .expect("claim should succeed");
        promote_claimed_work_to_doing(
            &store,
            &seeded.work_id,
            &seeded.agent_id,
            &claimed.lease.lease_id,
        );
        let expected_rev = store
            .load_context(&seeded.work_id)
            .expect("context should load")
            .snapshot
            .rev;

        let error = store
            .commit_decision(CommitDecisionReq::new(
                TransitionDecision {
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::StaleLease],
                    next_snapshot: None,
                    lease_effect: LeaseEffect::None,
                    pending_wake_effect: PendingWakeEffect::Retain,
                    gate_results: Vec::new(),
                    evidence: Default::default(),
                    summary: "stale lease".to_owned(),
                },
                TransitionRecord {
                    record_id: RecordId::from("record-stale-lease"),
                    company_id: seeded.company_id.clone(),
                    work_id: seeded.work_id.clone(),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(seeded.agent_id.as_str()),
                    run_id: claimed.lease.run_id.clone(),
                    session_id: None,
                    lease_id: Some(LeaseId::from("lease-missing")),
                    expected_rev,
                    contract_set_id: ContractSetId::from(seeded.contract_set_id.as_str()),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::StaleLease],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "stale lease".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: Vec::new(),
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "stale lease".to_owned(),
                    }),
                    evidence_refs: Vec::new(),
                    happened_at: timestamp(121),
                },
                None,
            ))
            .expect_err("stale lease should conflict");

        assert_eq!(error.kind, StoreErrorKind::Conflict);
        assert!(error.message.contains("lease"));
        assert!(store
            .select_record::<TransitionRecordDoc>(&transition_record_id("record-stale-lease"))
            .expect("transition record lookup should work")
            .is_none());
    }

    #[test]
    fn append_comment_persists_and_surfaces_on_work_read() {
        let store = new_store("append-comment");
        let seeded = seed_runtime_work(&store);

        let persisted = store
            .append_comment(AppendCommentReq {
                company_id: seeded.company_id.clone(),
                work_id: seeded.work_id.clone(),
                author_kind: ActorKind::Board,
                author_id: ActorId::from("board"),
                body: "board note".to_owned(),
            })
            .expect("comment should append");
        let work = store
            .read_work(Some(&seeded.work_id))
            .expect("work should read");

        assert!(persisted.comment_id.starts_with("comment-"));
        assert_eq!(work.items[0].comments.len(), 1);
        assert_eq!(work.items[0].comments[0].body, "board note");
        assert!(work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.event_kind == "comment" && entry.summary == "board note"));
    }

    #[test]
    fn read_models_reflect_live_store_state() {
        let store = new_store("read-models");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-read-models"),
            })
            .expect("claim should succeed");
        let sibling_work = create_todo_work(&store, &seeded.company_id, "Sibling");
        store
            .merge_wake(merge_wake_req(
                sibling_work.as_str(),
                "gate failed",
                &["cargo test"],
            ))
            .expect("wake should queue sibling run");
        store
            .record_consumption(RecordConsumptionReq {
                company_id: seeded.company_id.clone(),
                agent_id: seeded.agent_id.clone(),
                run_id: claimed
                    .lease
                    .run_id
                    .clone()
                    .expect("claim run should exist"),
                billing_kind: crate::model::BillingKind::Api,
                usage: crate::model::ConsumptionUsage {
                    input_tokens: 120,
                    output_tokens: 48,
                    run_seconds: 3,
                    estimated_cost_cents: Some(7),
                },
            })
            .expect("consumption should persist");
        store
            .save_session(&TaskSession {
                session_id: SessionId::from("session-queued"),
                company_id: seeded.company_id.clone(),
                agent_id: seeded.agent_id.clone(),
                work_id: sibling_work.clone(),
                runtime: RuntimeKind::Coclai,
                runtime_session_id: "runtime-queued".to_owned(),
                cwd: "/repo".to_owned(),
                workspace_fingerprint: workspace_fingerprint("/repo"),
                contract_rev: 1,
                last_record_id: None,
                last_decision_summary: Some("queued session".to_owned()),
                last_gate_summary: None,
                updated_at: timestamp(70),
            })
            .expect("session should persist");
        store
            .append_comment(AppendCommentReq {
                company_id: seeded.company_id.clone(),
                work_id: seeded.work_id.clone(),
                author_kind: ActorKind::Board,
                author_id: ActorId::from("board"),
                body: "audit note".to_owned(),
            })
            .expect("comment should append");

        let board = store.read_board();
        let work = store
            .read_work(Some(&sibling_work))
            .expect("work should read");
        let agents = store.read_agents();
        let activity = store.read_activity();

        assert!(board
            .running_agents
            .iter()
            .any(|agent| agent == seeded.agent_id.as_str()));
        assert!(board
            .running_runs
            .iter()
            .any(|run| run.lease_id.as_deref() == Some(claimed.lease.lease_id.as_str())));
        assert!(board
            .pending_wakes
            .iter()
            .any(|work_id| work_id == sibling_work.as_str()));
        assert!(board
            .pending_wake_details
            .iter()
            .any(|detail| detail.work_id == sibling_work.as_str()
                && detail.obligations == vec!["cargo test".to_owned()]));
        assert_eq!(board.consumption_summary.total_turns, 1);
        assert_eq!(board.consumption_summary.total_run_seconds, 3);
        assert_eq!(
            work.items[0].pending_obligations,
            vec!["cargo test".to_owned()]
        );
        assert!(agents
            .active_agents
            .iter()
            .any(|agent| agent == seeded.agent_id.as_str()));
        assert!(agents.recent_runs.iter().any(|run| run.status == "queued"));
        assert!(agents
            .current_sessions
            .iter()
            .any(|session| session.runtime_session_id == "runtime-queued"));
        assert_eq!(
            agents
                .consumption_by_agent
                .iter()
                .find(|summary| summary.agent_id == seeded.agent_id.as_str())
                .expect("agent rollup should exist")
                .total_estimated_cost_cents,
            7
        );
        assert!(activity
            .entries
            .iter()
            .any(|entry| entry.event_kind == "comment"));
        assert!(activity
            .entries
            .iter()
            .any(|entry| entry.event_kind == "run"));
    }

    #[test]
    fn read_board_projects_recent_gate_failure_details() {
        let store = new_store("board-gate-failure");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-board-failure"),
            })
            .expect("claim should succeed");
        promote_claimed_work_to_doing(
            &store,
            &seeded.work_id,
            &seeded.agent_id,
            &claimed.lease.lease_id,
        );
        let expected_rev = store
            .load_context(&seeded.work_id)
            .expect("context should load")
            .snapshot
            .rev;
        store
            .commit_decision(CommitDecisionReq::new(
                TransitionDecision {
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::GateFailed],
                    next_snapshot: None,
                    lease_effect: LeaseEffect::Keep,
                    pending_wake_effect: PendingWakeEffect::Retain,
                    gate_results: vec![GateResult {
                        gate: GateSpec::AllRequiredObligationsResolved,
                        passed: false,
                        detail: "all pending obligations must be resolved".to_owned(),
                    }],
                    evidence: Default::default(),
                    summary: "gate denied completion".to_owned(),
                },
                TransitionRecord {
                    record_id: RecordId::from("record-rejected"),
                    company_id: seeded.company_id.clone(),
                    work_id: seeded.work_id.clone(),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(seeded.agent_id.as_str()),
                    run_id: claimed.lease.run_id.clone(),
                    session_id: None,
                    lease_id: Some(claimed.lease.lease_id),
                    expected_rev,
                    contract_set_id: ContractSetId::from(seeded.contract_set_id.as_str()),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::GateFailed],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "attempt complete".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: vec![GateResult {
                        gate: GateSpec::AllRequiredObligationsResolved,
                        passed: false,
                        detail: "all pending obligations must be resolved".to_owned(),
                    }],
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "gate denied completion".to_owned(),
                    }),
                    evidence_refs: Vec::new(),
                    happened_at: timestamp(90),
                },
                None,
            ))
            .expect("rejected decision should persist");

        let board = store.read_board();

        assert_eq!(board.recent_gate_failures[0], "record-rejected");
        assert_eq!(
            board.recent_gate_failure_details[0].work_id,
            seeded.work_id.as_str()
        );
        assert_eq!(board.recent_gate_failure_details[0].outcome, "rejected");
        assert!(board.recent_gate_failure_details[0]
            .failed_gates
            .iter()
            .any(|detail| detail == "all pending obligations must be resolved"));
    }

    #[test]
    fn read_models_project_transition_record_details_into_board_and_work_audit() {
        let store = new_store("transition-projection");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-transition-projection"),
            })
            .expect("claim should succeed");
        promote_claimed_work_to_doing(
            &store,
            &seeded.work_id,
            &seeded.agent_id,
            &claimed.lease.lease_id,
        );
        let expected_rev = store
            .load_context(&seeded.work_id)
            .expect("context should load")
            .snapshot
            .rev;
        store
            .commit_decision(CommitDecisionReq::new(
                TransitionDecision {
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::GateFailed],
                    next_snapshot: None,
                    lease_effect: LeaseEffect::Keep,
                    pending_wake_effect: PendingWakeEffect::Retain,
                    gate_results: vec![GateResult {
                        gate: GateSpec::AllRequiredObligationsResolved,
                        passed: false,
                        detail: "all pending obligations must be resolved".to_owned(),
                    }],
                    evidence: Default::default(),
                    summary: "gate denied completion".to_owned(),
                },
                TransitionRecord {
                    record_id: RecordId::from("record-transition-projection"),
                    company_id: seeded.company_id.clone(),
                    work_id: seeded.work_id.clone(),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(seeded.agent_id.as_str()),
                    run_id: claimed.lease.run_id.clone(),
                    session_id: None,
                    lease_id: Some(claimed.lease.lease_id),
                    expected_rev,
                    contract_set_id: ContractSetId::from(seeded.contract_set_id.as_str()),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::GateFailed],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "attempt complete".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: vec![GateResult {
                        gate: GateSpec::AllRequiredObligationsResolved,
                        passed: false,
                        detail: "all pending obligations must be resolved".to_owned(),
                    }],
                    evidence: Default::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "gate denied completion".to_owned(),
                    }),
                    evidence_refs: Vec::new(),
                    happened_at: timestamp(122),
                },
                None,
            ))
            .expect("rejected decision should persist");

        let board = store.read_board();
        let work = store
            .read_work(Some(&seeded.work_id))
            .expect("work should read");

        assert_eq!(
            board.recent_transition_records[0],
            "record-transition-projection"
        );
        assert_eq!(
            board.recent_transition_details[0].record_id,
            "record-transition-projection"
        );
        assert_eq!(board.recent_transition_details[0].kind, "complete");
        assert_eq!(board.recent_transition_details[0].outcome, "rejected");
        assert_eq!(
            board.recent_transition_details[0].summary,
            "gate denied completion"
        );
        assert!(work.items[0].audit_entries.iter().any(|entry| {
            entry.event_kind == "transition"
                && entry.summary == "gate denied completion"
                && entry.outcome.as_deref() == Some("rejected")
                && entry.before_status == Some(WorkStatus::Doing)
                && entry.after_status.is_none()
        }));
    }

    #[test]
    fn commit_decision_persists_reason_codes_in_transition_record() {
        let store = new_store("record-reasons");
        let seeded = seed_runtime_work(&store);
        let claimed = store
            .claim_lease(ClaimLeaseReq {
                work_id: seeded.work_id.clone(),
                agent_id: seeded.agent_id.clone(),
                lease_id: LeaseId::from("lease-record-reasons"),
            })
            .expect("claim should succeed");
        promote_claimed_work_to_doing(
            &store,
            &seeded.work_id,
            &seeded.agent_id,
            &claimed.lease.lease_id,
        );
        let expected_rev = store
            .load_context(&seeded.work_id)
            .expect("context should load")
            .snapshot
            .rev;

        store
            .commit_decision(CommitDecisionReq::new(
                TransitionDecision {
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::GateFailed],
                    next_snapshot: None,
                    lease_effect: LeaseEffect::Keep,
                    pending_wake_effect: PendingWakeEffect::Retain,
                    gate_results: vec![GateResult {
                        gate: GateSpec::AllRequiredObligationsResolved,
                        passed: false,
                        detail: "all pending obligations must be resolved".to_owned(),
                    }],
                    evidence: Default::default(),
                    summary: "gate denied completion".to_owned(),
                },
                TransitionRecord {
                    record_id: RecordId::from("record-reasons"),
                    company_id: seeded.company_id.clone(),
                    work_id: seeded.work_id.clone(),
                    actor_kind: ActorKind::Agent,
                    actor_id: ActorId::from(seeded.agent_id.as_str()),
                    run_id: claimed.lease.run_id.clone(),
                    session_id: None,
                    lease_id: Some(claimed.lease.lease_id),
                    expected_rev,
                    contract_set_id: ContractSetId::from(seeded.contract_set_id.as_str()),
                    contract_rev: 1,
                    before_status: WorkStatus::Doing,
                    after_status: None,
                    outcome: DecisionOutcome::Rejected,
                    reasons: vec![crate::model::ReasonCode::GateFailed],
                    kind: TransitionKind::Complete,
                    patch: WorkPatch {
                        summary: "attempt complete".to_owned(),
                        resolved_obligations: Vec::new(),
                        declared_risks: Vec::new(),
                    },
                    gate_results: vec![GateResult {
                        gate: GateSpec::AllRequiredObligationsResolved,
                        passed: false,
                        detail: "all pending obligations must be resolved".to_owned(),
                    }],
                    evidence: EvidenceBundle::default(),
                    evidence_inline: Some(EvidenceInline {
                        summary: "gate denied completion".to_owned(),
                    }),
                    evidence_refs: Vec::new(),
                    happened_at: timestamp(123),
                },
                None,
            ))
            .expect("rejected decision should persist");

        let persisted = store
            .select_record::<TransitionRecordDoc>(&transition_record_id("record-reasons"))
            .expect("transition record lookup should work")
            .expect("transition record should persist");
        assert_eq!(persisted.reasons_json, serde_json::json!(["gate_failed"]));
    }

    #[test]
    fn timeout_replay_reconstructs_live_snapshot() {
        let store = new_store("timeout-replay");
        let seeded = seed_runtime_work(&store);
        store
            .merge_wake(merge_wake_req(
                seeded.work_id.as_str(),
                "retry timed out run",
                &["retry"],
            ))
            .expect("wake should seed pending follow-up");
        let before = store
            .select_record::<WorkDoc>(&work_record_id(seeded.work_id.as_str()))
            .expect("snapshot lookup should work")
            .expect("base snapshot should exist")
            .into_snapshot()
            .expect("base snapshot should decode");

        store
            .reap_timed_out_runs(Duration::from_secs(5))
            .expect("reaper should succeed");

        let live = store
            .select_record::<WorkDoc>(&work_record_id(seeded.work_id.as_str()))
            .expect("snapshot lookup should work")
            .expect("live snapshot should exist")
            .into_snapshot()
            .expect("live snapshot should decode");
        let pending_wake = store
            .select_record::<PendingWakeDoc>(&pending_wake_record_id(seeded.work_id.as_str()))
            .expect("pending wake lookup should work")
            .expect("pending wake should remain retained");
        let follow_up_run = store
            .select_record::<RunDoc>(&run_record_id("run-2"))
            .expect("follow-up lookup should work")
            .expect("follow-up queued run should persist");
        let replay_records = store
            .snapshot_state()
            .expect("snapshot state should load")
            .transition_records
            .into_iter()
            .filter(|record| {
                record.work_id == seeded.work_id.as_str()
                    && record.kind == super::transition_kind_label(TransitionKind::TimeoutRequeue)
            })
            .map(|record| TransitionRecord {
                record_id: RecordId::from(record.record_id),
                company_id: CompanyId::from(record.company_id),
                work_id: WorkId::from(record.work_id),
                actor_kind: super::parse_actor_kind(&record.actor_kind)
                    .expect("actor kind should parse"),
                actor_id: ActorId::from(record.actor_id),
                run_id: record.run_id.map(RunId::from),
                session_id: record.session_id.map(SessionId::from),
                lease_id: record.lease_id.map(LeaseId::from),
                expected_rev: record.expected_rev,
                contract_set_id: ContractSetId::from(record.contract_set_id),
                contract_rev: record.contract_rev,
                before_status: super::parse_work_status(&record.before_status)
                    .expect("before_status should parse"),
                after_status: record
                    .after_status
                    .as_deref()
                    .map(super::parse_work_status)
                    .transpose()
                    .expect("after_status should parse"),
                outcome: super::parse_decision_outcome(&record.outcome)
                    .expect("outcome should parse"),
                reasons: serde_json::from_value(record.reasons_json)
                    .expect("reasons should decode"),
                kind: super::parse_transition_kind(&record.kind).expect("kind should parse"),
                patch: WorkPatch {
                    summary: record.patch_summary,
                    resolved_obligations: record.resolved_obligations,
                    declared_risks: record.declared_risks,
                },
                gate_results: serde_json::from_value(record.gate_results_json)
                    .expect("gate results should decode"),
                evidence: serde_json::from_value(record.evidence_json)
                    .expect("evidence should decode"),
                evidence_inline: record
                    .evidence_summary
                    .map(|summary| EvidenceInline { summary }),
                evidence_refs: serde_json::from_value(record.evidence_refs_json)
                    .expect("evidence refs should decode"),
                happened_at: timestamp(record.happened_at_secs),
            })
            .collect::<Vec<_>>();
        let replayed = crate::kernel::replay_snapshot_from_records(&before, &replay_records)
            .expect("timeout replay should succeed");

        assert_eq!(pending_wake.count, 1);
        assert_eq!(follow_up_run.status, "queued");
        assert_eq!(replayed, live);
    }

    #[test]
    fn read_work_returns_recent_20_work_scoped_audit_entries() {
        let store = new_store("work-audit-cap");
        let seeded = seed_runtime_work(&store);
        let other = create_todo_work(&store, &seeded.company_id, "Noise");

        for seq in 0..25 {
            store
                .append_comment(AppendCommentReq {
                    company_id: seeded.company_id.clone(),
                    work_id: seeded.work_id.clone(),
                    author_kind: ActorKind::Board,
                    author_id: ActorId::from("board"),
                    body: format!("todo-{seq}"),
                })
                .expect("target comments should append");
        }
        for seq in 0..25 {
            store
                .append_comment(AppendCommentReq {
                    company_id: seeded.company_id.clone(),
                    work_id: other.clone(),
                    author_kind: ActorKind::Board,
                    author_id: ActorId::from("board"),
                    body: format!("noise-{seq}"),
                })
                .expect("noise comments should append");
        }

        let work = store
            .read_work(Some(&seeded.work_id))
            .expect("work should read");

        assert_eq!(work.items[0].audit_entries.len(), 20);
        assert!(work.items[0]
            .audit_entries
            .iter()
            .all(|entry| entry.work_id == seeded.work_id.as_str()));
        assert!(work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.summary == "todo-24"));
        assert!(!work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.summary == "todo-0"));
        assert!(!work.items[0]
            .audit_entries
            .iter()
            .any(|entry| entry.summary.starts_with("noise-")));
    }

    #[derive(Debug)]
    struct SeededRuntime {
        company_id: CompanyId,
        agent_id: AgentId,
        second_agent_id: AgentId,
        contract_set_id: ContractSetId,
        work_id: WorkId,
    }

    fn seed_runtime_work(store: &SurrealStore) -> SeededRuntime {
        let company = store
            .create_company(CreateCompanyReq {
                name: "Acme".to_owned(),
                description: "runtime".to_owned(),
                runtime_hard_stop_cents: None,
            })
            .expect("company should create");
        let agent = store
            .create_agent(CreateAgentReq {
                company_id: company.profile.company_id.clone(),
                name: "worker".to_owned(),
                role: "builder".to_owned(),
            })
            .expect("agent should create");
        let second_agent = store
            .create_agent(CreateAgentReq {
                company_id: company.profile.company_id.clone(),
                name: "backup".to_owned(),
                role: "reviewer".to_owned(),
            })
            .expect("backup agent should create");
        let draft = store
            .create_contract_draft(CreateContractDraftReq {
                company_id: company.profile.company_id.clone(),
                name: "axiomnexus-rust-next".to_owned(),
                rules: rules(),
            })
            .expect("draft should create");
        store
            .activate_contract(ActivateContractReq {
                company_id: company.profile.company_id.clone(),
                revision: draft.revision,
            })
            .expect("activation should succeed");
        let contracts = store.read_contracts();
        let work = store
            .create_work(CreateWorkReq {
                company_id: company.profile.company_id.clone(),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Runtime".to_owned(),
                body: "body".to_owned(),
                contract_set_id: ContractSetId::from(contracts.contract_set_id.as_str()),
            })
            .expect("work should create");
        store
            .upsert_record(
                &work_record_id(work.snapshot.work_id.as_str()),
                WorkDoc {
                    status: "todo".to_owned(),
                    ..WorkDoc::from_snapshot(&work.snapshot)
                },
            )
            .expect("runtime work should be promoted to todo");

        SeededRuntime {
            company_id: company.profile.company_id,
            agent_id: agent.agent_id,
            second_agent_id: second_agent.agent_id,
            contract_set_id: ContractSetId::from(contracts.contract_set_id.as_str()),
            work_id: work.snapshot.work_id,
        }
    }

    fn create_todo_work(store: &SurrealStore, company_id: &CompanyId, title: &str) -> WorkId {
        let contracts = store.read_contracts();
        let work = store
            .create_work(CreateWorkReq {
                company_id: company_id.clone(),
                parent_id: None,
                kind: WorkKind::Task,
                title: title.to_owned(),
                body: "body".to_owned(),
                contract_set_id: ContractSetId::from(contracts.contract_set_id.as_str()),
            })
            .expect("work should create");
        store
            .upsert_record(
                &work_record_id(work.snapshot.work_id.as_str()),
                WorkDoc {
                    status: "todo".to_owned(),
                    ..WorkDoc::from_snapshot(&work.snapshot)
                },
            )
            .expect("todo work should persist");
        work.snapshot.work_id
    }

    fn promote_claimed_work_to_doing(
        store: &SurrealStore,
        work_id: &WorkId,
        agent_id: &AgentId,
        lease_id: &LeaseId,
    ) {
        let existing = store
            .select_record::<WorkDoc>(&work_record_id(work_id.as_str()))
            .expect("work should load")
            .expect("work should exist");
        store
            .upsert_record(
                &work_record_id(work_id.as_str()),
                WorkDoc {
                    status: "doing".to_owned(),
                    assignee_agent_id: Some(agent_id.to_string()),
                    active_lease_id: Some(lease_id.to_string()),
                    rev: existing.rev + 1,
                    updated_at_secs: existing.updated_at_secs.saturating_add(1),
                    ..existing
                },
            )
            .expect("claimed work should become authoritative doing snapshot");
    }

    fn merge_wake_req(work_id: &str, reason: &str, obligations: &[&str]) -> MergeWakeReq {
        MergeWakeReq {
            work_id: WorkId::from(work_id),
            actor_kind: ActorKind::Board,
            actor_id: ActorId::from("board"),
            source: "manual".to_owned(),
            reason: reason.to_owned(),
            obligations: obligations.iter().map(|item| (*item).to_owned()).collect(),
        }
    }

    fn new_store(label: &str) -> SurrealStore {
        SurrealStore::open(&new_store_url(label)).expect("surreal store should open")
    }

    fn new_store_url(label: &str) -> String {
        format!(
            "surrealkv://{}/axiomnexus-surreal-{}-{}",
            std::env::temp_dir().display(),
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_nanos()
        )
    }

    fn snapshot_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "axiomnexus-surreal-snapshot-{}-{}.json",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_nanos()
        ))
    }

    fn rules() -> Vec<TransitionRule> {
        vec![TransitionRule {
            kind: TransitionKind::Queue,
            actor_kind: crate::model::ActorKind::Board,
            from: vec![WorkStatus::Backlog],
            to: WorkStatus::Todo,
            lease_effect: LeaseEffect::None,
            gates: Vec::new(),
        }]
    }
}
