mod bootstrap;
mod profile;
mod render;

use std::{path::Path, sync::Arc};

pub use bootstrap::AXIOMNEXUS_TRIAD_TOML as BOOTSTRAP_TRIAD_TOML;
use bootstrap::{bootstrap_repo, AXIOMNEXUS_TRIAD_TOML};
pub use profile::AxiomNexusProfile;
use render::{render_accept, render_init, render_next, render_status, render_verify, render_work};
use triad_config::{TriadConfig, CONFIG_FILE_NAME};
use triad_core::{
    ApplyPatchReport, ClaimId, NextClaim, PatchId, ReasoningLevel, RunClaimReport, RunClaimRequest,
    StatusReport, TriadApi, TriadError, VerifyReport, VerifyRequest,
};
use triad_runtime::LocalTriad;

pub fn run<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let command = parse_command(&args)?;
    let working_dir = std::env::current_dir()
        .map_err(|err| format!("failed to resolve current directory: {err}"))?;
    let output = execute(command, &working_dir)?;
    println!("{output}");
    Ok(())
}

fn execute(command: Command, working_dir: &Path) -> Result<String, String> {
    match command {
        Command::Init => {
            let repo_root = discover_repo_root_for_init(working_dir)?;
            bootstrap_repo(&repo_root).map_err(|err| err.to_string())?;
            Ok(render_init())
        }
        other => {
            let triad = load_runtime(working_dir)?;
            execute_against_runtime(other, &triad)
        }
    }
}

fn execute_against_runtime<R: GovernanceRuntime>(
    command: Command,
    runtime: &R,
) -> Result<String, String> {
    match command {
        Command::Init => Ok(render_init()),
        Command::Next => Ok(render_next(&runtime.next_claim().map_err(triad_error)?)),
        Command::Status { claim_id } => {
            let claim = parse_optional_claim_id(claim_id)?;
            Ok(render_status(
                &runtime.status(claim.as_ref()).map_err(triad_error)?,
            ))
        }
        Command::Work {
            claim_id,
            dry_run,
            model,
            effort,
        } => {
            let claim_id = resolve_claim_id(runtime, claim_id)?;
            let report = runtime
                .run_claim(RunClaimRequest {
                    claim_id,
                    dry_run,
                    model,
                    effort,
                })
                .map_err(triad_error)?;
            Ok(render_work(&report))
        }
        Command::Verify {
            claim_id,
            with_probe,
            full_workspace,
        } => {
            let claim_id = resolve_claim_id(runtime, claim_id)?;
            let request = runtime
                .default_verify_request(claim_id, with_probe, full_workspace)
                .map_err(triad_error)?;
            let report = runtime.verify_claim(request).map_err(triad_error)?;
            Ok(render_verify(&report))
        }
        Command::Accept { patch_id, latest } => {
            let patch_id = resolve_patch_id(runtime, patch_id, latest)?;
            let report = runtime.apply_patch(&patch_id).map_err(triad_error)?;
            Ok(render_accept(&report))
        }
    }
}

fn load_runtime(working_dir: &Path) -> Result<LocalTriad, String> {
    let repo_root = LocalTriad::discover_repo_root(working_dir)
        .map_err(|err| format!("failed to discover axiomnexus repo root: {err}"))?;
    let config = load_config(&repo_root)?;

    Ok(LocalTriad::with_profile(
        config,
        Arc::new(AxiomNexusProfile),
    ))
}

trait GovernanceRuntime {
    fn next_claim(&self) -> Result<NextClaim, TriadError>;
    fn status(&self, claim: Option<&ClaimId>) -> Result<StatusReport, TriadError>;
    fn run_claim(&self, req: RunClaimRequest) -> Result<RunClaimReport, TriadError>;
    fn default_verify_request(
        &self,
        claim_id: ClaimId,
        with_probe: bool,
        full_workspace: bool,
    ) -> Result<VerifyRequest, TriadError>;
    fn verify_claim(&self, req: VerifyRequest) -> Result<VerifyReport, TriadError>;
    fn latest_pending_patch_id(&self) -> Result<Option<PatchId>, TriadError>;
    fn apply_patch(&self, id: &PatchId) -> Result<ApplyPatchReport, TriadError>;
}

impl GovernanceRuntime for LocalTriad {
    fn next_claim(&self) -> Result<NextClaim, TriadError> {
        TriadApi::next_claim(self)
    }

    fn status(&self, claim: Option<&ClaimId>) -> Result<StatusReport, TriadError> {
        TriadApi::status(self, claim)
    }

    fn run_claim(&self, req: RunClaimRequest) -> Result<RunClaimReport, TriadError> {
        TriadApi::run_claim(self, req)
    }

    fn default_verify_request(
        &self,
        claim_id: ClaimId,
        with_probe: bool,
        full_workspace: bool,
    ) -> Result<VerifyRequest, TriadError> {
        LocalTriad::default_verify_request(self, claim_id, with_probe, full_workspace)
    }

    fn verify_claim(&self, req: VerifyRequest) -> Result<VerifyReport, TriadError> {
        TriadApi::verify_claim(self, req)
    }

    fn latest_pending_patch_id(&self) -> Result<Option<PatchId>, TriadError> {
        LocalTriad::latest_pending_patch_id(self)
    }

    fn apply_patch(&self, id: &PatchId) -> Result<ApplyPatchReport, TriadError> {
        TriadApi::apply_patch(self, id)
    }
}

fn load_config(repo_root: &Path) -> Result<triad_config::CanonicalTriadConfig, String> {
    TriadConfig::from_repo_root(repo_root)
        .map_err(triad_error)?
        .canonicalize(repo_root)
        .map_err(triad_error)?
        .validate()
        .map_err(triad_error)
}

fn discover_repo_root_for_init(working_dir: &Path) -> Result<std::path::PathBuf, String> {
    LocalTriad::discover_repo_root(working_dir).or_else(|_| {
        if working_dir.join(CONFIG_FILE_NAME).exists() || working_dir.join(".git").exists() {
            Ok(working_dir.to_path_buf())
        } else {
            let _ = TriadConfig::from_toml_str(AXIOMNEXUS_TRIAD_TOML).map_err(triad_error)?;
            Ok(working_dir.to_path_buf())
        }
    })
}

fn resolve_claim_id<R: GovernanceRuntime>(
    runtime: &R,
    raw: Option<String>,
) -> Result<ClaimId, String> {
    match raw {
        Some(raw) => ClaimId::new(&raw).map_err(triad_error),
        None => runtime
            .next_claim()
            .map(|next| next.claim_id)
            .map_err(triad_error),
    }
}

fn parse_optional_claim_id(raw: Option<String>) -> Result<Option<ClaimId>, String> {
    raw.map(|value| ClaimId::new(&value).map_err(triad_error))
        .transpose()
}

fn resolve_patch_id(
    runtime: &impl GovernanceRuntime,
    raw: Option<String>,
    latest: bool,
) -> Result<PatchId, String> {
    if latest {
        return runtime
            .latest_pending_patch_id()
            .map_err(triad_error)?
            .ok_or_else(|| "no pending patch is available".to_string());
    }

    let raw = raw.ok_or_else(|| "accept requires <patch-id> or --latest".to_string())?;
    PatchId::new(&raw).map_err(triad_error)
}

fn triad_error(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Init,
    Next,
    Status {
        claim_id: Option<String>,
    },
    Work {
        claim_id: Option<String>,
        dry_run: bool,
        model: Option<String>,
        effort: Option<ReasoningLevel>,
    },
    Verify {
        claim_id: Option<String>,
        with_probe: bool,
        full_workspace: bool,
    },
    Accept {
        patch_id: Option<String>,
        latest: bool,
    },
}

fn parse_command(args: &[String]) -> Result<Command, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };

    match command {
        "init" => {
            if args.len() == 1 {
                Ok(Command::Init)
            } else {
                Err(usage())
            }
        }
        "next" => {
            if args.len() == 1 {
                Ok(Command::Next)
            } else {
                Err(usage())
            }
        }
        "status" => parse_status(&args[1..]),
        "work" => parse_work(&args[1..]),
        "verify" => parse_verify(&args[1..]),
        "accept" => parse_accept(&args[1..]),
        _ => Err(usage()),
    }
}

fn parse_status(args: &[String]) -> Result<Command, String> {
    match args {
        [] => Ok(Command::Status { claim_id: None }),
        [claim_id] => Ok(Command::Status {
            claim_id: Some(claim_id.clone()),
        }),
        _ => Err(usage()),
    }
}

fn parse_work(args: &[String]) -> Result<Command, String> {
    let mut claim_id = None;
    let mut dry_run = false;
    let mut model = None;
    let mut effort = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                dry_run = true;
                index += 1;
            }
            "--model" => {
                let value = args.get(index + 1).ok_or_else(usage)?.to_string();
                model = Some(value);
                index += 2;
            }
            "--effort" => {
                let value = args.get(index + 1).ok_or_else(usage)?;
                effort = Some(parse_effort(value)?);
                index += 2;
            }
            value if value.starts_with("--") => return Err(usage()),
            value => {
                if claim_id.is_some() {
                    return Err(usage());
                }
                claim_id = Some(value.to_string());
                index += 1;
            }
        }
    }

    Ok(Command::Work {
        claim_id,
        dry_run,
        model,
        effort,
    })
}

fn parse_verify(args: &[String]) -> Result<Command, String> {
    let mut claim_id = None;
    let mut with_probe = false;
    let mut full_workspace = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--with-probe" => {
                with_probe = true;
                index += 1;
            }
            "--full-workspace" => {
                full_workspace = true;
                index += 1;
            }
            value if value.starts_with("--") => return Err(usage()),
            value => {
                if claim_id.is_some() {
                    return Err(usage());
                }
                claim_id = Some(value.to_string());
                index += 1;
            }
        }
    }

    Ok(Command::Verify {
        claim_id,
        with_probe,
        full_workspace,
    })
}

fn parse_accept(args: &[String]) -> Result<Command, String> {
    if args.len() == 1 && args[0] == "--latest" {
        return Ok(Command::Accept {
            patch_id: None,
            latest: true,
        });
    }

    if args.len() == 1 && !args[0].starts_with("--") {
        return Ok(Command::Accept {
            patch_id: Some(args[0].clone()),
            latest: false,
        });
    }

    Err(usage())
}

fn parse_effort(value: &str) -> Result<ReasoningLevel, String> {
    match value {
        "low" => Ok(ReasoningLevel::Low),
        "medium" => Ok(ReasoningLevel::Medium),
        "high" => Ok(ReasoningLevel::High),
        _ => Err("effort must be one of: low, medium, high".to_string()),
    }
}

fn usage() -> String {
    [
        "usage: axiomnexus-governance <command>",
        "",
        "commands:",
        "  init",
        "  next",
        "  status [claim-id]",
        "  work [claim-id] [--dry-run] [--model <name>] [--effort <low|medium|high>]",
        "  verify [claim-id] [--with-probe] [--full-workspace]",
        "  accept <patch-id>",
        "  accept --latest",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{execute_against_runtime, parse_command, parse_effort, Command, GovernanceRuntime};
    use triad_core::{
        ApplyPatchReport, ClaimId, DriftStatus, NextAction, NextClaim, PatchId, ReasoningLevel,
        RunClaimReport, RunClaimRequest, RunId, StatusReport, StatusSummary, Verdict, VerifyLayer,
        VerifyReport, VerifyRequest,
    };

    #[test]
    fn parser_accepts_documented_commands() {
        assert_eq!(
            parse_command(&["init".into()]).expect("init should parse"),
            Command::Init
        );
        assert_eq!(
            parse_command(&["next".into()]).expect("next should parse"),
            Command::Next
        );
        assert_eq!(
            parse_command(&["status".into(), "REQ-anx-001".into()]).expect("status should parse"),
            Command::Status {
                claim_id: Some("REQ-anx-001".to_string())
            }
        );
        assert_eq!(
            parse_command(&[
                "work".into(),
                "REQ-anx-001".into(),
                "--dry-run".into(),
                "--model".into(),
                "gpt-5-codex".into(),
                "--effort".into(),
                "high".into()
            ])
            .expect("work should parse"),
            Command::Work {
                claim_id: Some("REQ-anx-001".to_string()),
                dry_run: true,
                model: Some("gpt-5-codex".to_string()),
                effort: Some(ReasoningLevel::High),
            }
        );
        assert_eq!(
            parse_command(&["verify".into(), "--with-probe".into()]).expect("verify should parse"),
            Command::Verify {
                claim_id: None,
                with_probe: true,
                full_workspace: false,
            }
        );
        assert_eq!(
            parse_command(&["accept".into(), "--latest".into()]).expect("accept should parse"),
            Command::Accept {
                patch_id: None,
                latest: true,
            }
        );
    }

    #[test]
    fn parser_rejects_unknown_effort() {
        let error = parse_effort("ultra").expect_err("unknown effort should fail");
        assert_eq!(error, "effort must be one of: low, medium, high");
    }

    #[test]
    fn work_without_claim_id_uses_next_claim_and_forwards_request() {
        let runtime = FakeRuntime::default();

        let output = execute_against_runtime(
            Command::Work {
                claim_id: None,
                dry_run: true,
                model: Some("gpt-5-codex".to_string()),
                effort: Some(ReasoningLevel::High),
            },
            &runtime,
        )
        .expect("work should succeed");

        assert_eq!(runtime.next_claim_calls(), 1);
        let run_requests = runtime.run_requests();
        assert_eq!(run_requests.len(), 1);
        assert_eq!(
            run_requests[0].claim_id,
            ClaimId::new("REQ-anx-001").expect("claim id should parse")
        );
        assert!(run_requests[0].dry_run);
        assert_eq!(run_requests[0].model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(run_requests[0].effort, Some(ReasoningLevel::High));
        assert_eq!(
            output,
            "claim=REQ-anx-001 run=RUN-000001 needs_patch=true changed_paths=docs/07-triad-governance-integration-plan.md"
        );
    }

    #[test]
    fn accept_latest_resolves_pending_patch_before_apply() {
        let runtime = FakeRuntime::default();

        let output = execute_against_runtime(
            Command::Accept {
                patch_id: None,
                latest: true,
            },
            &runtime,
        )
        .expect("accept should succeed");

        assert_eq!(runtime.latest_patch_calls(), 1);
        assert_eq!(
            runtime.applied_patch_ids(),
            vec![PatchId::new("PATCH-000007").expect("patch id should parse")]
        );
        assert_eq!(
            output,
            "patch=PATCH-000007 claim=REQ-anx-001 applied=true revision=2 followup=Verify"
        );
    }

    #[derive(Default)]
    struct FakeRuntime {
        next_claim_calls: RefCell<u32>,
        latest_patch_calls: RefCell<u32>,
        run_requests: RefCell<Vec<RunClaimRequest>>,
        verify_requests: RefCell<Vec<VerifyRequest>>,
        applied_patch_ids: RefCell<Vec<PatchId>>,
    }

    impl FakeRuntime {
        fn next_claim_calls(&self) -> u32 {
            *self.next_claim_calls.borrow()
        }

        fn latest_patch_calls(&self) -> u32 {
            *self.latest_patch_calls.borrow()
        }

        fn run_requests(&self) -> Vec<RunClaimRequest> {
            self.run_requests.borrow().clone()
        }

        fn applied_patch_ids(&self) -> Vec<PatchId> {
            self.applied_patch_ids.borrow().clone()
        }
    }

    impl GovernanceRuntime for FakeRuntime {
        fn next_claim(&self) -> Result<NextClaim, triad_core::TriadError> {
            *self.next_claim_calls.borrow_mut() += 1;
            Ok(NextClaim {
                claim_id: ClaimId::new("REQ-anx-001").expect("claim id should parse"),
                status: DriftStatus::NeedsCode,
                reason: "needs implementation".to_string(),
                next_action: NextAction::Work,
            })
        }

        fn status(&self, _claim: Option<&ClaimId>) -> Result<StatusReport, triad_core::TriadError> {
            Ok(StatusReport {
                summary: StatusSummary {
                    healthy: 0,
                    needs_code: 1,
                    needs_test: 0,
                    needs_spec: 0,
                    contradicted: 0,
                    blocked: 0,
                },
                claims: Vec::new(),
            })
        }

        fn run_claim(
            &self,
            req: RunClaimRequest,
        ) -> Result<RunClaimReport, triad_core::TriadError> {
            self.run_requests.borrow_mut().push(req.clone());
            Ok(RunClaimReport {
                run_id: RunId::new("RUN-000001").expect("run id should parse"),
                claim_id: req.claim_id,
                summary: "updated governance docs".to_string(),
                changed_paths: vec!["docs/07-triad-governance-integration-plan.md".to_string()],
                suggested_test_selectors: vec![
                    "tests::canonical_asset_paths_are_used_consistently".to_string(),
                ],
                blocked_actions: Vec::new(),
                needs_patch: true,
            })
        }

        fn default_verify_request(
            &self,
            claim_id: ClaimId,
            with_probe: bool,
            full_workspace: bool,
        ) -> Result<VerifyRequest, triad_core::TriadError> {
            Ok(VerifyRequest {
                claim_id,
                layers: if with_probe {
                    vec![VerifyLayer::Unit, VerifyLayer::Probe]
                } else {
                    vec![VerifyLayer::Unit]
                },
                full_workspace,
            })
        }

        fn verify_claim(&self, req: VerifyRequest) -> Result<VerifyReport, triad_core::TriadError> {
            self.verify_requests.borrow_mut().push(req.clone());
            Ok(VerifyReport {
                claim_id: req.claim_id,
                verdict: Verdict::Pass,
                layers: req.layers,
                full_workspace: req.full_workspace,
                evidence_ids: Vec::new(),
                status_after_verify: DriftStatus::Healthy,
                pending_patch_id: None,
            })
        }

        fn latest_pending_patch_id(&self) -> Result<Option<PatchId>, triad_core::TriadError> {
            *self.latest_patch_calls.borrow_mut() += 1;
            Ok(Some(
                PatchId::new("PATCH-000007").expect("patch id should parse"),
            ))
        }

        fn apply_patch(&self, id: &PatchId) -> Result<ApplyPatchReport, triad_core::TriadError> {
            self.applied_patch_ids.borrow_mut().push(id.clone());
            Ok(ApplyPatchReport {
                patch_id: id.clone(),
                claim_id: ClaimId::new("REQ-anx-001").expect("claim id should parse"),
                applied: true,
                new_revision: 2,
                followup_action: NextAction::Verify,
            })
        }
    }
}
