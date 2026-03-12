use std::{
    io::{self, Write},
    path::Path,
};

use crate::{
    adapter::{
        coclai::assets::RuntimeAssets,
        http::{routes::all_routes, server::serve as serve_http, transport::HttpTransport},
        memory::store::{DEMO_AGENT_ID, DEMO_DOING_WORK_ID, DEMO_LEASE_ID},
        surreal::store::{SurrealStore, DEFAULT_DATABASE, DEFAULT_NAMESPACE},
        workspace::SystemWorkspace,
    },
    app::cmd::{
        submit_intent::{handle_submit_intent, SubmitIntentCmd},
        RUNTIME_RESUME_POLICY,
    },
    model::{
        AgentId, ContractSetStatus, LeaseId, ProofHint, ProofHintKind, TransitionIntent,
        TransitionKind, WorkId, WorkPatch,
    },
    port::store::{CommandStorePort, CompanyReadModel, ContractsReadModel, QueryStorePort},
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
            writeln!(
                stdout,
                "axiomnexus doctor live; data dir: {}; store_url={}; runtime_policy={}; transport=tcp-http",
                config.data_dir.display(),
                store_url,
                RUNTIME_RESUME_POLICY
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
    store: &impl CommandStorePort,
    config: &Config,
    stdout: &mut impl Write,
) -> Result<(), BootError> {
    let submit = handle_submit_intent(
        store,
        &SystemWorkspace,
        SubmitIntentCmd {
            intent: sample_intent(),
        },
    )
    .map_err(|error| io::Error::other(error.to_string()))?;
    writeln!(
        stdout,
        "axiomnexus replay live; data dir: {}; store_url={}; decision_path={}; outcome={:?}; store_backed_replay=live",
        config.data_dir.display(),
        config.store_url,
        submit.decision_path,
        submit.outcome
    )?;
    Ok(())
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

fn sample_intent() -> TransitionIntent {
    TransitionIntent {
        work_id: WorkId::from(DEMO_DOING_WORK_ID),
        agent_id: AgentId::from(DEMO_AGENT_ID),
        lease_id: LeaseId::from(DEMO_LEASE_ID),
        expected_rev: 1,
        kind: TransitionKind::ProposeProgress,
        patch: WorkPatch {
            summary: "sample".to_owned(),
            resolved_obligations: Vec::new(),
            declared_risks: Vec::new(),
        },
        note: None,
        proof_hints: vec![ProofHint {
            kind: ProofHintKind::Summary,
            value: "sample".to_owned(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        model::{
            ActorKind, ContractSetStatus, LeaseEffect, TransitionKind, TransitionRule, WorkStatus,
        },
        port::store::{CompanySummaryView, ContractRevisionView},
    };

    use super::{validate_contract_check_models, ContractCheckSummary};

    #[test]
    fn contract_check_accepts_active_live_contract_bound_to_company() {
        let companies = crate::port::store::CompanyReadModel {
            items: vec![CompanySummaryView {
                company_id: "company-1".to_owned(),
                name: "Demo".to_owned(),
                description: "demo".to_owned(),
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
