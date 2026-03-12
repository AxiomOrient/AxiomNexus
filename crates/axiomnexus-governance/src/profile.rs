use triad_config::CanonicalTriadConfig;
use triad_core::{TriadError, VerifyLayer, VerifyRequest};
use triad_runtime::{
    BuildWorkContext, GovernanceProfile, GovernanceWorkContext, PromptAttachment, VerifyCommandPlan,
};

#[derive(Debug, Default)]
pub struct AxiomNexusProfile;

impl GovernanceProfile for AxiomNexusProfile {
    fn build_work_context(
        &self,
        input: BuildWorkContext<'_>,
    ) -> Result<GovernanceWorkContext, TriadError> {
        Ok(GovernanceWorkContext {
            prompt_attachments: vec![
                PromptAttachment::AtPath {
                    path: "AGENTS.md".to_string(),
                    placeholder: None,
                },
                PromptAttachment::AtPath {
                    path: input.claim_path.as_str().to_string(),
                    placeholder: None,
                },
                PromptAttachment::AtPath {
                    path: "docs/05-target-architecture.md".to_string(),
                    placeholder: None,
                },
                PromptAttachment::AtPath {
                    path: "samples/transition-intent.schema.json".to_string(),
                    placeholder: None,
                },
            ],
            allowed_write_roots: vec![
                input.config.repo_root.join("src"),
                input.config.repo_root.join("docs"),
                input.config.repo_root.join("samples"),
                input.config.repo_root.join(".agents"),
                input.config.repo_root.join("README.md"),
                input.config.repo_root.join("Cargo.toml"),
                input.config.repo_root.join("Cargo.lock"),
            ],
            protected_write_roots: vec![input.config.paths.claim_dir.clone()],
        })
    }

    fn plan_verify_commands(
        &self,
        _config: &CanonicalTriadConfig,
        req: &VerifyRequest,
        selectors: &[String],
    ) -> Result<Vec<VerifyCommandPlan>, TriadError> {
        let mut plans = Vec::new();

        for layer in req.layers.iter().copied() {
            match layer {
                VerifyLayer::Unit => {
                    if selectors.is_empty() {
                        plans.push(VerifyCommandPlan {
                            layer,
                            command: "cargo test --lib".to_string(),
                            targeted: false,
                        });
                    } else {
                        for selector in selectors {
                            plans.push(VerifyCommandPlan {
                                layer,
                                command: format!("cargo test {selector}"),
                                targeted: true,
                            });
                        }
                    }
                }
                VerifyLayer::Contract => {
                    plans.push(VerifyCommandPlan {
                        layer,
                        command: "cargo fmt --all --check".to_string(),
                        targeted: false,
                    });
                    plans.push(VerifyCommandPlan {
                        layer,
                        command: "cargo test --lib canonical_asset_paths_are_used_consistently"
                            .to_string(),
                        targeted: false,
                    });
                    plans.push(VerifyCommandPlan {
                        layer,
                        command: "cargo run -- contract check".to_string(),
                        targeted: false,
                    });
                }
                VerifyLayer::Integration => {
                    plans.push(VerifyCommandPlan {
                        layer,
                        command: "cargo clippy --all-targets --all-features -- -D warnings"
                            .to_string(),
                        targeted: false,
                    });
                    plans.push(VerifyCommandPlan {
                        layer,
                        command: "cargo test".to_string(),
                        targeted: false,
                    });
                }
                VerifyLayer::Probe => {}
            }
        }

        Ok(plans)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use triad_config::TriadConfig;
    use triad_core::{ClaimId, VerifyLayer, VerifyRequest};
    use triad_runtime::GovernanceProfile;

    use super::AxiomNexusProfile;
    use crate::bootstrap::AXIOMNEXUS_TRIAD_TOML;

    #[test]
    fn axiomnexus_profile_builds_expected_work_context() {
        let temp = TestDir::new("axiomnexus-profile-context");
        let repo_root = temp.path();
        let claim_dir = repo_root.join("spec/claims");
        fs::create_dir_all(&claim_dir).expect("claim dir should exist");
        fs::write(
            claim_dir.join("REQ-anx-001.md"),
            "# REQ-anx-001 Sample\n\n## Claim\nSample.\n\n## Examples\n- one\n\n## Invariants\n- one\n",
        )
        .expect("claim file should exist");

        let config = TriadConfig::from_toml_str(AXIOMNEXUS_TRIAD_TOML)
            .expect("config should parse")
            .canonicalize(repo_root)
            .expect("config should canonicalize");
        let profile = AxiomNexusProfile;
        let claim_id = ClaimId::new("REQ-anx-001").expect("claim id should parse");
        let claim_path = config.paths.claim_dir.join("REQ-anx-001.md");
        let claim_relative = claim_path
            .strip_prefix(&config.repo_root)
            .expect("claim path should stay within repo root");
        let context = profile
            .build_work_context(triad_runtime::BuildWorkContext {
                config: &config,
                claim_id: &claim_id,
                claim_path: claim_relative,
            })
            .expect("work context should build");

        assert_eq!(context.prompt_attachments.len(), 4);
        assert_eq!(
            context.prompt_attachments[0],
            triad_runtime::PromptAttachment::AtPath {
                path: "AGENTS.md".to_string(),
                placeholder: None,
            }
        );
        assert_eq!(
            context.prompt_attachments[1],
            triad_runtime::PromptAttachment::AtPath {
                path: "spec/claims/REQ-anx-001.md".to_string(),
                placeholder: None,
            }
        );
        assert_eq!(
            context.prompt_attachments[2],
            triad_runtime::PromptAttachment::AtPath {
                path: "docs/05-target-architecture.md".to_string(),
                placeholder: None,
            }
        );
        assert_eq!(
            context.prompt_attachments[3],
            triad_runtime::PromptAttachment::AtPath {
                path: "samples/transition-intent.schema.json".to_string(),
                placeholder: None,
            }
        );
        assert!(context
            .allowed_write_roots
            .iter()
            .any(|path| path.ends_with("src")));
        assert!(context
            .allowed_write_roots
            .iter()
            .any(|path| path.ends_with("README.md")));
        assert_eq!(context.protected_write_roots, vec![config.paths.claim_dir]);
    }

    #[test]
    fn axiomnexus_profile_plans_expected_verify_commands() {
        let config = TriadConfig::from_toml_str(AXIOMNEXUS_TRIAD_TOML)
            .expect("config should parse")
            .canonicalize("/")
            .expect("config should canonicalize");
        let profile = AxiomNexusProfile;

        let targeted = profile
            .plan_verify_commands(
                &config,
                &VerifyRequest {
                    claim_id: ClaimId::new("REQ-anx-001").expect("claim id should parse"),
                    layers: vec![VerifyLayer::Unit],
                    full_workspace: false,
                },
                &["kernel::tests::replay_reconstructs_timeout_requeue_snapshot_chain".to_string()],
            )
            .expect("targeted verify plan should resolve");
        let layered = profile
            .plan_verify_commands(
                &config,
                &VerifyRequest {
                    claim_id: ClaimId::new("REQ-anx-001").expect("claim id should parse"),
                    layers: vec![VerifyLayer::Contract, VerifyLayer::Integration],
                    full_workspace: true,
                },
                &[],
            )
            .expect("layered verify plan should resolve");

        assert_eq!(targeted.len(), 1);
        assert_eq!(
            targeted[0].command,
            "cargo test kernel::tests::replay_reconstructs_timeout_requeue_snapshot_chain"
        );
        assert!(targeted[0].targeted);
        assert_eq!(
            layered
                .iter()
                .map(|plan| plan.command.as_str())
                .collect::<Vec<_>>(),
            vec![
                "cargo fmt --all --check",
                "cargo test --lib canonical_asset_paths_are_used_consistently",
                "cargo run -- contract check",
                "cargo clippy --all-targets --all-features -- -D warnings",
                "cargo test",
            ]
        );
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should advance")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "axiomnexus-governance-{label}-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("temp dir should exist");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
