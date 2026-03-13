use std::{
    io::{self, Write},
    path::Path,
};

use crate::{
    adapter::{
        coclai::assets::{
            RuntimeAssets, AGENTS_ASSET_PATH, TRANSITION_EXECUTOR_SKILL_PATH,
            TRANSITION_INTENT_SCHEMA_PATH,
        },
        http::{routes::all_routes, server::serve as serve_http, transport::HttpTransport},
        surreal::store::{SurrealStore, DEFAULT_DATABASE, DEFAULT_NAMESPACE},
    },
    app::cmd::RUNTIME_RESUME_POLICY,
    model::ContractSetStatus,
    port::store::{CompanyReadModel, ContractsReadModel, QueryStorePort, ReplayStorePort},
};

use super::{cli::Command, config::Config, BootError};

pub fn dispatch(command: Command, config: &Config) -> Result<(), BootError> {
    let mut stdout = io::stdout().lock();

    match command {
        Command::Serve => {
            let store_url = surreal_store_url(config)?;
            let routes = all_routes();
            writeln!(
                stdout,
                "axiomnexus serve live; data dir: {}; store_url={}; bind_addr={}; route_manifest={}; http_server=tcp",
                config.data_dir.display(),
                store_url,
                config.http_bind_addr,
                routes.len(),
            )?;
            stdout.flush()?;
            let transport = HttpTransport::new(SurrealStore::open(store_url)?);
            serve_http(transport, &config.http_bind_addr)?;
        }
        Command::Migrate => {
            let store_url = surreal_store_url(config)?;
            SurrealStore::migrate(store_url)?;
            writeln!(
                stdout,
                "axiomnexus migrate live; data dir: {}; store_url={}; namespace={}; database={}; storage path: adapter::surreal::store",
                config.data_dir.display(),
                store_url,
                DEFAULT_NAMESPACE,
                DEFAULT_DATABASE
            )?;
        }
        Command::Doctor => {
            let store_url = surreal_store_url(config)?;
            let _store = SurrealStore::open(store_url)?;
            let assets = doctor_asset_summary(Path::new(env!("CARGO_MANIFEST_DIR")))
                .map_err(|error| io::Error::other(error.to_string()))?;
            writeln!(
                stdout,
                "axiomnexus doctor live; data dir: {}; store_url={}; runtime_policy={}; transport=tcp-http; assets_loaded=yes; agents_asset_path={}; agents_bytes={}; skill_asset_path={}; skill_bytes={}; schema_path={}; schema_bytes={}",
                config.data_dir.display(),
                store_url,
                RUNTIME_RESUME_POLICY,
                AGENTS_ASSET_PATH,
                assets.agents_bytes,
                TRANSITION_EXECUTOR_SKILL_PATH,
                assets.skill_bytes,
                TRANSITION_INTENT_SCHEMA_PATH,
                assets.schema_bytes
            )?;
        }
        Command::Replay => {
            let store_url = surreal_store_url(config)?;
            let store = SurrealStore::open(store_url)?;
            replay_with_store(&store, config, &mut stdout)?;
        }
        Command::Export => {
            let store_url = surreal_store_url(config)?;
            let export = SurrealStore::export_snapshot_to_file(store_url, &config.export_path)?;
            writeln!(
                stdout,
                "axiomnexus export live; data dir: {}; store_url={}; export_path={}; checksum_fnv64={}; format={}",
                config.data_dir.display(),
                store_url,
                config.export_path.display(),
                export.checksum_fnv64,
                export.format
            )?;
        }
        Command::Import => {
            let store_url = surreal_store_url(config)?;
            let export = SurrealStore::import_snapshot_from_file(store_url, &config.export_path)?;
            writeln!(
                stdout,
                "axiomnexus import live; data dir: {}; store_url={}; export_path={}; checksum_fnv64={}; format={}",
                config.data_dir.display(),
                store_url,
                config.export_path.display(),
                export.checksum_fnv64,
                export.format
            )?;
        }
        Command::ContractCheck => {
            let store_url = surreal_store_url(config)?;
            let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
                .map_err(|error| io::Error::other(error.to_string()))?;
            let store = SurrealStore::open(store_url)?;
            let summary = contract_check_summary(&store, assets.transition_intent_schema.len())?;
            writeln!(
                stdout,
                "axiomnexus contract check live; data dir: {}; store_url={}; contract_set_id={}; revision={}; rules={}; bound_companies={}; schema path: samples/transition-intent.schema.json; schema_bytes={}; assets_loaded=yes",
                config.data_dir.display(),
                store_url,
                summary.contract_set_id,
                summary.revision,
                summary.rule_count,
                summary.bound_company_count,
                assets.transition_intent_schema.len()
            )?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContractCheckSummary {
    contract_set_id: String,
    revision: u32,
    rule_count: usize,
    bound_company_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplaySummary {
    total_work_count: usize,
    verified_work_count: usize,
    skipped_work_count: usize,
    replayed_record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DoctorAssetSummary {
    agents_bytes: usize,
    skill_bytes: usize,
    schema_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayFailureDetail {
    work_id: String,
    record_id: Option<String>,
    reason_code: String,
    message: String,
}

fn contract_check_summary(
    store: &impl QueryStorePort,
    schema_bytes: usize,
) -> Result<ContractCheckSummary, BootError> {
    validate_contract_check_models(
        &store.read_companies(),
        &store.read_contracts(),
        schema_bytes,
    )
}

fn doctor_asset_summary(repo_root: &Path) -> Result<DoctorAssetSummary, BootError> {
    let assets = RuntimeAssets::load_from_repo_root(repo_root).map_err(|error| {
        BootError::Runtime(format!(
            "doctor could not load canonical runtime assets: {error}"
        ))
    })?;

    Ok(DoctorAssetSummary {
        agents_bytes: assets.agents_md.len(),
        skill_bytes: assets.transition_executor_skill.len(),
        schema_bytes: assets.transition_intent_schema.len(),
    })
}

fn validate_contract_check_models(
    companies: &CompanyReadModel,
    contracts: &ContractsReadModel,
    schema_bytes: usize,
) -> Result<ContractCheckSummary, BootError> {
    if schema_bytes == 0 {
        return Err(BootError::Runtime(
            "contract check requires a non-empty transition intent schema asset".to_owned(),
        ));
    }

    if contracts.contract_set_id.trim().is_empty() {
        return Err(BootError::Store(
            "contract check could not resolve an active contract set from the live store"
                .to_owned(),
        ));
    }

    if contracts.status != ContractSetStatus::Active {
        return Err(BootError::Store(
            "contract check requires an active contract revision in the live store".to_owned(),
        ));
    }

    if contracts.revision == 0 {
        return Err(BootError::Store(
            "contract check requires a non-zero active contract revision".to_owned(),
        ));
    }

    if contracts.rules.is_empty() {
        return Err(BootError::Store(
            "contract check requires at least one transition rule in the active contract"
                .to_owned(),
        ));
    }

    if !contracts.revisions.iter().any(|revision| {
        revision.revision == contracts.revision && revision.status == ContractSetStatus::Active
    }) {
        return Err(BootError::Store(
            "contract check could not match the selected active revision in contract history"
                .to_owned(),
        ));
    }

    let bound_company_count = companies
        .items
        .iter()
        .filter(|company| {
            company.active_contract_set_id.as_deref() == Some(contracts.contract_set_id.as_str())
                && company.active_contract_revision == Some(contracts.revision)
        })
        .count();
    if bound_company_count == 0 {
        return Err(BootError::Store(
            "contract check requires at least one company bound to the selected active contract"
                .to_owned(),
        ));
    }

    Ok(ContractCheckSummary {
        contract_set_id: contracts.contract_set_id.clone(),
        revision: contracts.revision,
        rule_count: contracts.rules.len(),
        bound_company_count,
    })
}

fn replay_with_store(
    store: &impl ReplayStorePort,
    config: &Config,
    stdout: &mut impl Write,
) -> Result<(), BootError> {
    let summary = replay_store(store)?;
    writeln!(
        stdout,
        "axiomnexus replay live; data dir: {}; store_url={}; decision_path=transition_record; verified_work_count={}; skipped_work_count={}; replayed_record_count={}; total_work_count={}; store_backed_replay=live",
        config.data_dir.display(),
        config.store_url,
        summary.verified_work_count,
        summary.skipped_work_count,
        summary.replayed_record_count,
        summary.total_work_count
    )?;
    Ok(())
}

fn replay_store(store: &impl ReplayStorePort) -> Result<ReplaySummary, BootError> {
    let snapshots = store.list_work_snapshots()?;
    let mut verified_work_count = 0;
    let mut skipped_work_count = 0;
    let mut replayed_record_count = 0;

    for snapshot in &snapshots {
        let records = store.load_transition_records(&snapshot.work_id)?;
        if records.is_empty() {
            if crate::kernel::replay_base_snapshot(snapshot) == *snapshot {
                verified_work_count += 1;
            } else {
                skipped_work_count += 1;
            }
            continue;
        }

        let replayed = crate::kernel::replay_snapshot_from_records(
            &crate::kernel::replay_base_snapshot(snapshot),
            &records,
        )
        .map_err(|error| replay_failure(&snapshot.work_id, error))?;
        if replayed != *snapshot {
            return Err(replay_failure(
                &snapshot.work_id,
                crate::kernel::replay_snapshot_mismatch(
                    records.last().map(|record| record.record_id.clone()),
                    "replayed snapshot does not match live snapshot",
                ),
            ));
        }

        verified_work_count += 1;
        replayed_record_count += records.len();
    }

    Ok(ReplaySummary {
        total_work_count: snapshots.len(),
        verified_work_count,
        skipped_work_count,
        replayed_record_count,
    })
}

fn replay_failure(work_id: &crate::model::WorkId, error: crate::kernel::ReplayError) -> BootError {
    let detail = ReplayFailureDetail {
        work_id: work_id.to_string(),
        record_id: error.record_id.map(|record_id| record_id.to_string()),
        reason_code: error.code.as_str().to_owned(),
        message: error.message,
    };
    BootError::Store(format!(
        "axiomnexus replay mismatch; work_id={}; record_id={}; reason_code={}; detail={}",
        detail.work_id,
        detail.record_id.as_deref().unwrap_or("-"),
        detail.reason_code,
        detail.message
    ))
}

fn surreal_store_url(config: &Config) -> Result<&str, BootError> {
    if config.store_url.starts_with("surrealkv://") {
        Ok(config.store_url.as_str())
    } else {
        Err(BootError::Store(format!(
            "live commands require surrealkv:// target store_url: {}",
            config.store_url
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        path::Path,
        time::{Duration, SystemTime},
    };

    use crate::{
        adapter::coclai::assets::{
            AGENTS_ASSET_PATH, TRANSITION_EXECUTOR_SKILL_PATH, TRANSITION_INTENT_SCHEMA_PATH,
        },
        model::{
            ActorId, ActorKind, ContractSetId, ContractSetStatus, DecisionOutcome, LeaseEffect,
            Priority, RecordId, TransitionKind, TransitionRecord, TransitionRule, WorkId,
            WorkPatch, WorkSnapshot, WorkStatus,
        },
        port::store::{CompanySummaryView, ContractRevisionView, ReplayStorePort, StoreError},
    };

    use super::{
        doctor_asset_summary, replay_failure, replay_store, validate_contract_check_models,
        ContractCheckSummary, DoctorAssetSummary, ReplaySummary,
    };

    #[test]
    fn contract_check_accepts_active_live_contract_bound_to_company() {
        let companies = crate::port::store::CompanyReadModel {
            items: vec![CompanySummaryView {
                company_id: "company-1".to_owned(),
                name: "Demo".to_owned(),
                description: "demo".to_owned(),
                runtime_hard_stop_cents: None,
                active_contract_set_id: Some("contract-1".to_owned()),
                active_contract_revision: Some(3),
                agent_count: 1,
                work_count: 1,
            }],
        };
        let contracts = crate::port::store::ContractsReadModel {
            contract_set_id: "contract-1".to_owned(),
            name: "demo-contract".to_owned(),
            revision: 3,
            status: ContractSetStatus::Active,
            revisions: vec![ContractRevisionView {
                revision: 3,
                status: ContractSetStatus::Active,
                name: "demo-contract".to_owned(),
            }],
            rules: vec![sample_rule()],
        };

        let summary = validate_contract_check_models(&companies, &contracts, 128)
            .expect("active contract check should pass");

        assert_eq!(
            summary,
            ContractCheckSummary {
                contract_set_id: "contract-1".to_owned(),
                revision: 3,
                rule_count: 1,
                bound_company_count: 1,
            }
        );
    }

    #[test]
    fn contract_check_rejects_contract_not_bound_to_any_company() {
        let companies = crate::port::store::CompanyReadModel {
            items: vec![CompanySummaryView {
                company_id: "company-1".to_owned(),
                name: "Demo".to_owned(),
                description: "demo".to_owned(),
                runtime_hard_stop_cents: None,
                active_contract_set_id: Some("other-contract".to_owned()),
                active_contract_revision: Some(3),
                agent_count: 1,
                work_count: 1,
            }],
        };
        let contracts = crate::port::store::ContractsReadModel {
            contract_set_id: "contract-1".to_owned(),
            name: "demo-contract".to_owned(),
            revision: 3,
            status: ContractSetStatus::Active,
            revisions: vec![ContractRevisionView {
                revision: 3,
                status: ContractSetStatus::Active,
                name: "demo-contract".to_owned(),
            }],
            rules: vec![sample_rule()],
        };

        let error = validate_contract_check_models(&companies, &contracts, 128)
            .expect_err("unbound active contract should fail");

        assert!(error
            .to_string()
            .contains("at least one company bound to the selected active contract"));
    }

    #[test]
    fn contract_check_rejects_non_active_or_empty_rule_contracts() {
        let companies = crate::port::store::CompanyReadModel {
            items: vec![CompanySummaryView {
                company_id: "company-1".to_owned(),
                name: "Demo".to_owned(),
                description: "demo".to_owned(),
                runtime_hard_stop_cents: None,
                active_contract_set_id: Some("contract-1".to_owned()),
                active_contract_revision: Some(2),
                agent_count: 1,
                work_count: 1,
            }],
        };
        let draft_contracts = crate::port::store::ContractsReadModel {
            contract_set_id: "contract-1".to_owned(),
            name: "draft-contract".to_owned(),
            revision: 2,
            status: ContractSetStatus::Draft,
            revisions: vec![ContractRevisionView {
                revision: 2,
                status: ContractSetStatus::Draft,
                name: "draft-contract".to_owned(),
            }],
            rules: Vec::new(),
        };

        let error = validate_contract_check_models(&companies, &draft_contracts, 128)
            .expect_err("draft contract should fail");

        assert!(error
            .to_string()
            .contains("requires an active contract revision"));
    }

    #[test]
    fn doctor_asset_summary_loads_canonical_runtime_assets() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let summary = doctor_asset_summary(repo_root).expect("doctor asset summary should load");

        assert_eq!(
            summary,
            DoctorAssetSummary {
                agents_bytes: std::fs::read_to_string(repo_root.join(AGENTS_ASSET_PATH))
                    .expect("agents asset should load")
                    .len(),
                skill_bytes: std::fs::read_to_string(
                    repo_root.join(TRANSITION_EXECUTOR_SKILL_PATH)
                )
                .expect("skill asset should load")
                .len(),
                schema_bytes: std::fs::read_to_string(
                    repo_root.join(TRANSITION_INTENT_SCHEMA_PATH)
                )
                .expect("schema asset should load")
                .len(),
            }
        );
    }

    #[derive(Default)]
    struct FakeReplayStore {
        snapshots: Vec<WorkSnapshot>,
        records: BTreeMap<String, Vec<TransitionRecord>>,
    }

    impl ReplayStorePort for FakeReplayStore {
        fn list_work_snapshots(&self) -> Result<Vec<WorkSnapshot>, StoreError> {
            Ok(self.snapshots.clone())
        }

        fn load_transition_records(
            &self,
            work_id: &crate::model::WorkId,
        ) -> Result<Vec<TransitionRecord>, StoreError> {
            Ok(self
                .records
                .get(work_id.as_str())
                .cloned()
                .unwrap_or_default())
        }
    }

    #[test]
    fn replay_store_verifies_full_transition_record_stream() {
        let work_id = WorkId::from("work-1");
        let base = base_snapshot(&work_id);
        let live = WorkSnapshot {
            status: WorkStatus::Doing,
            rev: 2,
            assignee_agent_id: Some(crate::model::AgentId::from("agent-1")),
            active_lease_id: Some(crate::model::LeaseId::from("lease-1")),
            updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
            ..base.clone()
        };
        let queue_record = TransitionRecord {
            record_id: RecordId::from("record-1"),
            company_id: base.company_id.clone(),
            work_id: work_id.clone(),
            actor_kind: ActorKind::Board,
            actor_id: ActorId::from("board"),
            run_id: None,
            session_id: None,
            lease_id: None,
            expected_rev: 0,
            contract_set_id: base.contract_set_id.clone(),
            contract_rev: base.contract_rev,
            before_status: WorkStatus::Backlog,
            after_status: Some(WorkStatus::Todo),
            outcome: DecisionOutcome::Accepted,
            reasons: Vec::new(),
            kind: TransitionKind::Queue,
            patch: WorkPatch::default(),
            gate_results: Vec::new(),
            evidence: crate::model::EvidenceBundle::default(),
            evidence_inline: None,
            evidence_refs: Vec::new(),
            happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        };
        let claim_record = TransitionRecord {
            record_id: RecordId::from("record-2"),
            company_id: base.company_id.clone(),
            work_id: work_id.clone(),
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::from("agent-1"),
            run_id: Some(crate::model::RunId::from("run-1")),
            session_id: None,
            lease_id: Some(crate::model::LeaseId::from("lease-1")),
            expected_rev: 1,
            contract_set_id: base.contract_set_id.clone(),
            contract_rev: base.contract_rev,
            before_status: WorkStatus::Todo,
            after_status: Some(WorkStatus::Doing),
            outcome: DecisionOutcome::Accepted,
            reasons: Vec::new(),
            kind: TransitionKind::Claim,
            patch: WorkPatch::default(),
            gate_results: Vec::new(),
            evidence: crate::model::EvidenceBundle::default(),
            evidence_inline: None,
            evidence_refs: Vec::new(),
            happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
        };
        let rejected_complete_record = TransitionRecord {
            record_id: RecordId::from("record-3"),
            company_id: base.company_id.clone(),
            work_id: work_id.clone(),
            actor_kind: ActorKind::Agent,
            actor_id: ActorId::from("agent-1"),
            run_id: Some(crate::model::RunId::from("run-1")),
            session_id: None,
            lease_id: Some(crate::model::LeaseId::from("lease-1")),
            expected_rev: 2,
            contract_set_id: base.contract_set_id.clone(),
            contract_rev: base.contract_rev,
            before_status: WorkStatus::Doing,
            after_status: None,
            outcome: DecisionOutcome::Rejected,
            reasons: vec![crate::model::ReasonCode::GateFailed],
            kind: TransitionKind::Complete,
            patch: WorkPatch::default(),
            gate_results: Vec::new(),
            evidence: crate::model::EvidenceBundle::default(),
            evidence_inline: None,
            evidence_refs: Vec::new(),
            happened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(3),
        };
        let store = FakeReplayStore {
            snapshots: vec![live],
            records: BTreeMap::from([(
                work_id.to_string(),
                vec![queue_record, claim_record, rejected_complete_record],
            )]),
        };

        let summary = replay_store(&store).expect("replay should succeed");

        assert_eq!(
            summary,
            ReplaySummary {
                total_work_count: 1,
                verified_work_count: 1,
                skipped_work_count: 0,
                replayed_record_count: 3,
            }
        );
    }

    #[test]
    fn replay_store_skips_non_initial_work_without_records() {
        let store = FakeReplayStore {
            snapshots: vec![WorkSnapshot {
                status: WorkStatus::Doing,
                rev: 1,
                assignee_agent_id: Some(crate::model::AgentId::from("agent-1")),
                active_lease_id: Some(crate::model::LeaseId::from("lease-1")),
                updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
                ..base_snapshot(&WorkId::from("work-2"))
            }],
            records: BTreeMap::new(),
        };

        let summary = replay_store(&store).expect("recordless non-initial work should be skipped");

        assert_eq!(summary.total_work_count, 1);
        assert_eq!(summary.verified_work_count, 0);
        assert_eq!(summary.skipped_work_count, 1);
        assert_eq!(summary.replayed_record_count, 0);
    }

    #[test]
    fn replay_failure_reports_work_record_and_reason_code() {
        let error = replay_failure(
            &WorkId::from("work-9"),
            crate::kernel::replay_snapshot_mismatch(
                Some(RecordId::from("record-9")),
                "replayed snapshot does not match live snapshot",
            ),
        );

        let rendered = error.to_string();

        assert!(rendered.contains("work_id=work-9"));
        assert!(rendered.contains("record_id=record-9"));
        assert!(rendered.contains("reason_code=snapshot_mismatch"));
    }

    fn base_snapshot(work_id: &WorkId) -> WorkSnapshot {
        WorkSnapshot {
            work_id: work_id.clone(),
            company_id: crate::model::CompanyId::from("company-1"),
            parent_id: None,
            kind: crate::model::WorkKind::Task,
            title: "Demo".to_owned(),
            body: String::new(),
            status: WorkStatus::Backlog,
            priority: Priority::Medium,
            assignee_agent_id: None,
            active_lease_id: None,
            rev: 0,
            contract_set_id: ContractSetId::from("contract-1"),
            contract_rev: 1,
            created_at: SystemTime::UNIX_EPOCH,
            updated_at: SystemTime::UNIX_EPOCH,
        }
    }

    fn sample_rule() -> TransitionRule {
        TransitionRule {
            kind: TransitionKind::Queue,
            actor_kind: ActorKind::Board,
            from: vec![WorkStatus::Backlog],
            to: WorkStatus::Todo,
            lease_effect: LeaseEffect::None,
            gates: Vec::new(),
        }
    }
}
