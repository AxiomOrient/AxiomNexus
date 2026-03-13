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
}
