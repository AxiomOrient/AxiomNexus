pub(crate) mod cmd;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    #[test]
    fn app_layer_workspace_evidence_stays_behind_port_boundary() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let app_dir = repo_root.join("src/app");
        let app_cmd_dir = app_dir.join("cmd");
        let forbidden_tokens = [
            "std::fs",
            "std::process::Command",
            "git2",
            "adapter::workspace",
            "std::env::current_dir",
        ];

        for path in rust_files_under(&app_cmd_dir) {
            let display_path = path
                .strip_prefix(repo_root)
                .expect("app path should stay under repo root");
            let text = fs::read_to_string(&path).expect("source file should load");
            for token in forbidden_tokens {
                assert!(
                    !text.contains(token),
                    "{} should not bypass WorkspacePort with forbidden token {token}",
                    display_path.display()
                );
            }
        }

        let submit_intent = fs::read_to_string(app_dir.join("cmd/submit_intent.rs"))
            .expect("submit_intent source should load");
        assert!(submit_intent.contains("workspace::WorkspacePort"));
        assert!(submit_intent.contains("workspace.observe_changed_files"));
        assert!(submit_intent.contains("workspace.run_gate_command"));
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
