use std::{fs, io, path::Path};

pub(crate) const AGENTS_ASSET_PATH: &str = ".agents/AGENTS.md";
pub(crate) const TRANSITION_EXECUTOR_SKILL_PATH: &str =
    ".agents/skills/transition-executor/SKILL.md";
pub(crate) const TRANSITION_INTENT_SCHEMA_PATH: &str = "samples/transition-intent.schema.json";
pub(crate) const EXECUTE_TURN_OUTPUT_SCHEMA_PATH: &str = "samples/execute-turn-output.schema.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAssets {
    pub(crate) agents_md: String,
    pub(crate) transition_executor_skill: String,
    pub(crate) transition_intent_schema: String,
    pub(crate) execute_turn_output_schema: String,
}

impl RuntimeAssets {
    pub(crate) fn load_from_repo_root(repo_root: &Path) -> io::Result<Self> {
        Ok(Self {
            agents_md: fs::read_to_string(repo_root.join(AGENTS_ASSET_PATH))?,
            transition_executor_skill: fs::read_to_string(
                repo_root.join(TRANSITION_EXECUTOR_SKILL_PATH),
            )?,
            transition_intent_schema: fs::read_to_string(
                repo_root.join(TRANSITION_INTENT_SCHEMA_PATH),
            )?,
            execute_turn_output_schema: fs::read_to_string(
                repo_root.join(EXECUTE_TURN_OUTPUT_SCHEMA_PATH),
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        RuntimeAssets, AGENTS_ASSET_PATH, EXECUTE_TURN_OUTPUT_SCHEMA_PATH,
        TRANSITION_EXECUTOR_SKILL_PATH, TRANSITION_INTENT_SCHEMA_PATH,
    };

    #[test]
    fn runtime_assets_load_from_canonical_repo_paths() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let assets = RuntimeAssets::load_from_repo_root(repo_root).expect("assets should load");

        assert!(repo_root.join(AGENTS_ASSET_PATH).exists());
        assert!(repo_root.join(TRANSITION_EXECUTOR_SKILL_PATH).exists());
        assert!(repo_root.join(TRANSITION_INTENT_SCHEMA_PATH).exists());
        assert!(repo_root.join(EXECUTE_TURN_OUTPUT_SCHEMA_PATH).exists());
        assert!(assets.agents_md.contains("TransitionIntent"));
        assert!(assets
            .transition_executor_skill
            .contains("Output JSON only"));
        assert!(assets
            .transition_intent_schema
            .contains("\"title\": \"TransitionIntent\""));
        assert!(assets
            .execute_turn_output_schema
            .contains("\"title\": \"ExecuteTurnOutcome\""));
    }
}
