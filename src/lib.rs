pub mod boot;

pub(crate) mod adapter;
pub(crate) mod app;
pub(crate) mod kernel;
pub(crate) mod model;
pub(crate) mod port;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use serde_json::Value;

    const CANONICAL_EXECUTE_TURN_SCHEMA_PATH: &str = "samples/execute-turn-output.schema.json";
    const CANONICAL_TRANSITION_SCHEMA_PATH: &str = "samples/transition-intent.schema.json";
    const CANONICAL_DEMO_CONTRACT_SAMPLE_PATH: &str = "samples/company-rust-contract.example.json";
    const CANONICAL_AGENTS_ASSET_PATH: &str = ".agents/AGENTS.md";
    const CANONICAL_TRANSITION_EXECUTOR_SKILL_PATH: &str =
        ".agents/skills/transition-executor/SKILL.md";

    #[test]
    fn top_level_module_tree_matches_canonical_layout() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut dirs = fs::read_dir(src)
            .expect("src dir should exist")
            .filter_map(|entry| {
                let entry = entry.ok()?;
                entry.file_type().ok().filter(|ty| ty.is_dir())?;
                Some(entry.file_name().to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>();
        dirs.sort();

        assert_eq!(
            dirs,
            vec!["adapter", "app", "boot", "kernel", "model", "port"]
        );
    }

    #[test]
    fn kernel_and_model_stay_free_of_forbidden_dependencies() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let forbidden_tokens = [
            "tokio",
            "sqlx",
            "reqwest",
            "std::fs",
            "std::process::Command",
        ];

        for path in rust_files_under(&repo_root.join("src/kernel")) {
            let display_path = path
                .strip_prefix(repo_root)
                .expect("kernel path should stay under repo root");
            let text = fs::read_to_string(&path).expect("source file should load");
            for token in forbidden_tokens {
                assert!(
                    !text.contains(token),
                    "{} should not depend on forbidden token {token}",
                    display_path.display()
                );
            }
        }

        for path in rust_files_under(&repo_root.join("src/model")) {
            let display_path = path
                .strip_prefix(repo_root)
                .expect("model path should stay under repo root");
            let text = fs::read_to_string(&path).expect("source file should load");
            for token in forbidden_tokens {
                assert!(
                    !text.contains(token),
                    "{} should not depend on forbidden token {token}",
                    display_path.display()
                );
            }
        }
    }

    #[test]
    fn canonical_asset_paths_are_used_consistently() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let schema_ref_files = [
            "README.md",
            "AGENTS.md",
            ".agents/AGENTS.md",
            "src/adapter/coclai/assets.rs",
            "src/adapter/coclai/contract.rs",
            "src/adapter/coclai/runtime.rs",
            "src/boot/wire.rs",
        ];

        assert!(repo_root.join(CANONICAL_AGENTS_ASSET_PATH).exists());
        assert!(repo_root
            .join(CANONICAL_TRANSITION_EXECUTOR_SKILL_PATH)
            .exists());
        assert!(repo_root.join(CANONICAL_EXECUTE_TURN_SCHEMA_PATH).exists());
        assert!(repo_root.join(CANONICAL_TRANSITION_SCHEMA_PATH).exists());
        assert!(repo_root.join(CANONICAL_DEMO_CONTRACT_SAMPLE_PATH).exists());

        for path in schema_ref_files {
            let text = fs::read_to_string(repo_root.join(path)).expect("source file should load");
            assert!(
                text.contains(CANONICAL_TRANSITION_SCHEMA_PATH),
                "{path} should reference the canonical transition schema path",
            );
        }

        for path in [
            "README.md",
            "docs/01-FINAL-TARGET.md",
            "docs/spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md",
        ] {
            let text = fs::read_to_string(repo_root.join(path)).expect("source file should load");
            assert!(
                text.contains(CANONICAL_EXECUTE_TURN_SCHEMA_PATH),
                "{path} should reference the canonical execute-turn schema path",
            );
        }

        for path in [
            "src/adapter/memory/store.rs",
            "src/adapter/surreal/store.rs",
        ] {
            let text = fs::read_to_string(repo_root.join(path)).expect("source file should load");
            assert!(
                text.contains(CANONICAL_DEMO_CONTRACT_SAMPLE_PATH),
                "{path} should reference the canonical demo contract sample path",
            );
        }
    }

    #[test]
    fn canonical_docs_surface_is_present() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let required_docs = [
            "docs/00-index.md",
            "docs/01-FINAL-TARGET.md",
            "docs/02-BLUEPRINT.md",
            "docs/03-DOMAIN-AND-INVARIANTS.md",
            "docs/04-API-SURFACE.md",
            "docs/05-QUALITY-GATES.md",
            "docs/spec/STOREPORT-SEMANTIC-CONTRACT.md",
            "docs/spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md",
            "docs/spec/CONFORMANCE-SUITE.md",
            "docs/adr/ADR-003-remove-workspaceport.md",
            "docs/adr/ADR-004-surreal-first-postgres-later.md",
        ];

        for path in required_docs {
            assert!(repo_root.join(path).exists(), "{path} should exist");
        }

        let index =
            fs::read_to_string(repo_root.join("docs/00-index.md")).expect("docs index should load");
        for path in [
            "01-FINAL-TARGET.md",
            "02-BLUEPRINT.md",
            "03-DOMAIN-AND-INVARIANTS.md",
            "04-API-SURFACE.md",
            "05-QUALITY-GATES.md",
            "spec/STOREPORT-SEMANTIC-CONTRACT.md",
            "spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md",
            "spec/CONFORMANCE-SUITE.md",
        ] {
            assert!(
                index.contains(path),
                "docs index should mention the promoted canonical path {path}",
            );
        }
    }

    #[test]
    fn execute_turn_contract_includes_gate_plan_and_observations() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let runtime_port = fs::read_to_string(repo_root.join("src/port/runtime.rs"))
            .expect("runtime port should load");
        let schema = fs::read_to_string(repo_root.join(CANONICAL_EXECUTE_TURN_SCHEMA_PATH))
            .expect("execute turn schema should load");

        assert!(runtime_port.contains("pub(crate) gate_plan: Vec<GateCommandSpec>"));
        assert!(runtime_port.contains("pub(crate) observations: RuntimeObservations"));
        assert!(schema.contains("\"observations\""));
    }

    #[test]
    fn transition_intent_schema_gate_is_live_contract() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let schema = load_schema(repo_root, CANONICAL_TRANSITION_SCHEMA_PATH);

        assert_eq!(schema["title"], "TransitionIntent");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["required"]
            .as_array()
            .expect("required should be an array")
            .iter()
            .any(|item| item == "work_id"));
        assert!(schema["required"]
            .as_array()
            .expect("required should be an array")
            .iter()
            .any(|item| item == "proof_hints"));
        assert_eq!(schema["properties"]["kind"]["enum"][0], "propose_progress");
        assert_eq!(schema["properties"]["kind"]["enum"][2], "block");
    }

    #[test]
    fn execute_turn_output_schema_gate_is_live_contract() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let schema = load_schema(repo_root, CANONICAL_EXECUTE_TURN_SCHEMA_PATH);

        assert_eq!(schema["title"], "ExecuteTurnOutcome");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["required"]
            .as_array()
            .expect("required should be an array")
            .iter()
            .any(|item| item == "observations"));
        assert_eq!(
            schema["properties"]["result"]["properties"]["intent"]["title"],
            "TransitionIntent"
        );
        assert!(schema["properties"]["observations"]["required"]
            .as_array()
            .expect("required should be an array")
            .iter()
            .any(|item| item == "command_results"));
    }

    #[test]
    fn cargo_manifest_stays_single_root_package() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest =
            fs::read_to_string(repo_root.join("Cargo.toml")).expect("cargo manifest should load");

        assert!(manifest.contains("name = \"axiomnexus\""));
        assert!(manifest.contains("[package]"));
        assert!(!manifest.contains("[workspace]"));
        assert!(!manifest.contains("crates/axiomsync"));
    }

    #[test]
    fn workspace_port_boundary_stays_removed_from_runtime_path() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));

        assert!(
            !repo_root.join("src/port/workspace.rs").exists(),
            "workspace port file should stay removed",
        );

        let port_mod =
            fs::read_to_string(repo_root.join("src/port/mod.rs")).expect("port module should load");
        assert!(
            !port_mod.contains("workspace"),
            "port module should not re-export workspace boundary",
        );

        let run_turn_once = fs::read_to_string(repo_root.join("src/app/cmd/run_turn_once.rs"))
            .expect("run_turn_once should load");
        assert!(
            !run_turn_once.contains("WorkspacePort"),
            "run_turn_once should depend on runtime observations instead of WorkspacePort",
        );

        let guarded_dirs = [repo_root.join("src/app"), repo_root.join("src/port")];
        for dir in guarded_dirs {
            for path in rust_files_under(&dir) {
                let display_path = path
                    .strip_prefix(repo_root)
                    .expect("source path should stay under repo root");
                let text = fs::read_to_string(&path).expect("source file should load");
                for token in [
                    "workspace::WorkspacePort",
                    "port::workspace",
                    "mod workspace",
                ] {
                    assert!(
                        !text.contains(token),
                        "{} should not contain removed workspace token {token}",
                        display_path.display()
                    );
                }
            }
        }
    }

    fn rust_files_under(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let entries = fs::read_dir(dir).expect("source dir should exist");
        for entry in entries {
            let entry = entry.expect("dir entry should load");
            let path = entry.path();
            if entry.file_type().expect("file type should load").is_dir() {
                files.extend(rust_files_under(&path));
                continue;
            }

            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
        files.sort();
        files
    }

    fn load_schema(repo_root: &Path, path: &str) -> Value {
        serde_json::from_str(
            &fs::read_to_string(repo_root.join(path)).expect("schema file should load"),
        )
        .expect("schema should parse as json")
    }
}
