use std::{
    fs,
    path::{Path, PathBuf},
};

use triad_core::TriadError;
use triad_runtime::export_embedded_runtime_schemas;

pub const AXIOMNEXUS_TRIAD_TOML: &str = include_str!("../../../triad.toml");

const CLAIM_SEEDS: &[(&str, &str)] = &[
    (
        "REQ-anx-001.md",
        include_str!("../../../spec/claims/REQ-anx-001.md"),
    ),
    (
        "REQ-anx-002.md",
        include_str!("../../../spec/claims/REQ-anx-002.md"),
    ),
    (
        "REQ-anx-003.md",
        include_str!("../../../spec/claims/REQ-anx-003.md"),
    ),
    (
        "REQ-anx-004.md",
        include_str!("../../../spec/claims/REQ-anx-004.md"),
    ),
    (
        "REQ-anx-005.md",
        include_str!("../../../spec/claims/REQ-anx-005.md"),
    ),
    (
        "REQ-anx-006.md",
        include_str!("../../../spec/claims/REQ-anx-006.md"),
    ),
];

pub fn bootstrap_repo(repo_root: &Path) -> Result<(), TriadError> {
    ensure_text_file(repo_root.join("triad.toml"), AXIOMNEXUS_TRIAD_TOML)?;
    ensure_dir(repo_root.join("docs"))?;
    ensure_dir(repo_root.join("spec/claims"))?;
    ensure_dir(repo_root.join(".triad"))?;
    ensure_dir(repo_root.join(".triad/patches"))?;
    ensure_dir(repo_root.join(".triad/runs"))?;
    ensure_file(repo_root.join(".triad/evidence.ndjson"))?;

    for (name, contents) in CLAIM_SEEDS {
        ensure_text_file(repo_root.join("spec/claims").join(name), contents)?;
    }

    export_embedded_runtime_schemas(&repo_root.join(".triad/schemas"))?;
    Ok(())
}

fn ensure_dir(path: PathBuf) -> Result<(), TriadError> {
    fs::create_dir_all(&path)
        .map_err(|err| TriadError::Io(format!("failed to create dir {}: {err}", path.display())))
}

fn ensure_file(path: PathBuf) -> Result<(), TriadError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            TriadError::Io(format!(
                "failed to create parent dir {}: {err}",
                parent.display()
            ))
        })?;
    }
    fs::write(&path, "")
        .map_err(|err| TriadError::Io(format!("failed to create file {}: {err}", path.display())))
}

fn ensure_text_file(path: PathBuf, contents: &str) -> Result<(), TriadError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            TriadError::Io(format!(
                "failed to create parent dir {}: {err}",
                parent.display()
            ))
        })?;
    }
    fs::write(&path, contents)
        .map_err(|err| TriadError::Io(format!("failed to write file {}: {err}", path.display())))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{bootstrap_repo, AXIOMNEXUS_TRIAD_TOML};
    use triad_config::TriadConfig;

    #[test]
    fn bootstrap_repo_creates_expected_axiomnexus_triage_layout() {
        let temp = TestDir::new("bootstrap");
        bootstrap_repo(temp.path()).expect("bootstrap should succeed");

        assert!(temp.path().join("triad.toml").is_file());
        assert!(temp.path().join("docs").is_dir());
        assert!(temp.path().join("spec/claims/REQ-anx-001.md").is_file());
        assert!(temp.path().join(".triad/evidence.ndjson").is_file());
        assert!(temp.path().join(".triad/patches").is_dir());
        assert!(temp.path().join(".triad/runs").is_dir());
        assert!(temp
            .path()
            .join(".triad/schemas/agent.run.schema.json")
            .is_file());
    }

    #[test]
    fn bootstrap_toml_parses_as_valid_triad_config() {
        let config =
            TriadConfig::from_toml_str(AXIOMNEXUS_TRIAD_TOML).expect("bootstrap toml should parse");

        assert_eq!(config.paths.claim_dir.as_str(), "spec/claims");
        assert_eq!(config.paths.schema_dir.as_str(), ".triad/schemas");
        assert_eq!(config.agent.backend.as_str(), "codex");
        assert_eq!(
            config.verify.default_layers,
            vec!["unit", "contract", "integration"]
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
                "axiomnexus-governance-bootstrap-{label}-{}-{stamp}",
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
