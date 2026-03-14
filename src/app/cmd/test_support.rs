use std::path::Path;

use crate::{adapter::coclai::assets::RuntimeAssets, model::ConsumptionUsage};

pub(crate) fn load_runtime_assets() -> RuntimeAssets {
    RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("assets should load")
}

pub(crate) fn sample_usage(
    input_tokens: u64,
    output_tokens: u64,
    run_seconds: u64,
    estimated_cost_cents: u64,
) -> ConsumptionUsage {
    ConsumptionUsage {
        input_tokens,
        output_tokens,
        run_seconds,
        estimated_cost_cents: Some(estimated_cost_cents),
    }
}

pub(crate) struct RuntimeIntentOutput<'a> {
    pub(crate) work_id: &'a str,
    pub(crate) agent_id: &'a str,
    pub(crate) lease_id: &'a str,
    pub(crate) expected_rev: u64,
    pub(crate) kind: &'a str,
    pub(crate) summary: &'a str,
    pub(crate) note: Option<&'a str>,
    pub(crate) proof_hints: &'a [(&'a str, &'a str)],
}

pub(crate) fn runtime_intent_output(output: RuntimeIntentOutput<'_>) -> String {
    let proof_hints = output
        .proof_hints
        .iter()
        .map(|(kind, value)| serde_json::json!({ "kind": kind, "value": value }))
        .collect::<Vec<_>>();

    serde_json::json!({
        "work_id": output.work_id,
        "agent_id": output.agent_id,
        "lease_id": output.lease_id,
        "expected_rev": output.expected_rev,
        "kind": output.kind,
        "patch": {
            "summary": output.summary,
            "resolved_obligations": [],
            "declared_risks": [],
        },
        "note": output.note,
        "proof_hints": proof_hints,
    })
    .to_string()
}
