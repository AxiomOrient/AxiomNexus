use crate::{
    adapter::coclai::assets::TRANSITION_INTENT_SCHEMA_PATH,
    model::{TransitionIntent, TransitionKind},
    port::runtime::{RuntimeError, RuntimeErrorKind},
};

pub(crate) const INVALID_OUTPUT_REPAIR_BUDGET: u8 = 1;
const MAX_TEXT_LEN: usize = 4000;
const MAX_LIST_ITEMS: usize = 64;

pub(crate) fn output_rule_line() -> String {
    format!(
        "Output rule: valid JSON for TransitionIntent using schema at {TRANSITION_INTENT_SCHEMA_PATH}."
    )
}

pub(crate) fn validate_runtime_output(
    raw_output: &str,
    intent: &TransitionIntent,
) -> Result<(), RuntimeError> {
    let parsed = serde_json::from_str::<TransitionIntent>(raw_output).map_err(|error| {
        invalid_output(&format!(
            "failed to parse runtime output as TransitionIntent: {error}"
        ))
    })?;

    validate_schema_contract(&parsed)?;

    if expected_runtime_kind(intent.kind).is_none() || !parsed.kind.is_runtime_intent() {
        return Err(invalid_output(
            "runtime output must use runtime intent kinds only",
        ));
    }

    if parsed.kind != intent.kind {
        return Err(invalid_output(
            "kind does not match normalized transition intent",
        ));
    }

    if parsed.kind == TransitionKind::Block
        && parsed
            .note
            .as_deref()
            .is_none_or(|note| note.trim().is_empty())
    {
        return Err(invalid_output("block output requires non-empty note"));
    }

    if parsed != *intent {
        return Err(invalid_output(
            "raw output does not match normalized transition intent",
        ));
    }

    Ok(())
}

fn expected_runtime_kind(kind: TransitionKind) -> Option<&'static str> {
    match kind {
        TransitionKind::ProposeProgress => Some("propose_progress"),
        TransitionKind::Complete => Some("complete"),
        TransitionKind::Block => Some("block"),
        TransitionKind::Queue
        | TransitionKind::Claim
        | TransitionKind::Reopen
        | TransitionKind::Cancel
        | TransitionKind::OverrideComplete
        | TransitionKind::TimeoutRequeue => None,
    }
}

fn validate_schema_contract(intent: &TransitionIntent) -> Result<(), RuntimeError> {
    validate_uuid("work_id", intent.work_id.as_str())?;
    validate_uuid("agent_id", intent.agent_id.as_str())?;
    validate_uuid("lease_id", intent.lease_id.as_str())?;
    validate_text("patch.summary", &intent.patch.summary, true)?;
    validate_text_option("note", intent.note.as_deref())?;
    validate_string_list(
        "patch.resolved_obligations",
        &intent.patch.resolved_obligations,
    )?;
    validate_string_list("patch.declared_risks", &intent.patch.declared_risks)?;

    if intent.proof_hints.len() > MAX_LIST_ITEMS {
        return Err(invalid_output("proof_hints exceeds maxItems=64"));
    }

    for proof_hint in &intent.proof_hints {
        validate_text("proof_hints[].value", &proof_hint.value, true)?;
    }

    Ok(())
}

fn validate_uuid(field: &str, value: &str) -> Result<(), RuntimeError> {
    if !is_uuid(value) {
        return Err(invalid_output(&format!("{field} must use uuid format")));
    }

    Ok(())
}

fn validate_string_list(field: &str, values: &[String]) -> Result<(), RuntimeError> {
    if values.len() > MAX_LIST_ITEMS {
        return Err(invalid_output(&format!("{field} exceeds maxItems=64")));
    }

    for value in values {
        validate_text(field, value, true)?;
    }

    Ok(())
}

fn validate_text_option(field: &str, value: Option<&str>) -> Result<(), RuntimeError> {
    if let Some(value) = value {
        validate_text(field, value, false)?;
    }

    Ok(())
}

fn validate_text(field: &str, value: &str, reject_blank: bool) -> Result<(), RuntimeError> {
    if reject_blank && value.trim().is_empty() {
        return Err(invalid_output(&format!("{field} must be non-empty")));
    }

    if value.len() > MAX_TEXT_LEN {
        return Err(invalid_output(&format!("{field} exceeds maxLength=4000")));
    }

    Ok(())
}

fn is_uuid(value: &str) -> bool {
    const HYPHEN_INDEXES: [usize; 4] = [8, 13, 18, 23];

    if value.len() != 36 {
        return false;
    }

    value.chars().enumerate().all(|(index, ch)| {
        if HYPHEN_INDEXES.contains(&index) {
            ch == '-'
        } else {
            ch.is_ascii_hexdigit()
        }
    })
}

fn invalid_output(message: &str) -> RuntimeError {
    RuntimeError {
        kind: RuntimeErrorKind::InvalidOutput,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        model::{
            AgentId, LeaseId, ProofHint, ProofHintKind, TransitionIntent, TransitionKind, WorkId,
            WorkPatch,
        },
        port::runtime::RuntimeErrorKind,
    };

    use super::{output_rule_line, validate_runtime_output};

    #[test]
    fn output_rule_mentions_canonical_schema_path() {
        assert!(output_rule_line().contains("samples/transition-intent.schema.json"));
    }

    #[test]
    fn validate_runtime_output_rejects_missing_required_field() {
        let error = validate_runtime_output(
            "{\"kind\":\"complete\"}",
            &intent(TransitionKind::Complete, Some("done".to_owned())),
        )
        .expect_err("missing fields should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error.message.contains("missing field"));
    }

    #[test]
    fn validate_runtime_output_rejects_block_without_note() {
        let error =
            validate_runtime_output(&valid_output("block"), &intent(TransitionKind::Block, None))
                .expect_err("block without note should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error
            .message
            .contains("block output requires non-empty note"));
    }

    #[test]
    fn validate_runtime_output_rejects_malformed_json() {
        let error = validate_runtime_output(
            "not-json",
            &intent(TransitionKind::Complete, Some("done".to_owned())),
        )
        .expect_err("malformed json should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error.message.contains("failed to parse runtime output"));
    }

    #[test]
    fn validate_runtime_output_rejects_unknown_fields() {
        let error = validate_runtime_output(
            &valid_output_with_extra_field("complete"),
            &intent(TransitionKind::Complete, Some("done".to_owned())),
        )
        .expect_err("unexpected fields should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error.message.contains("unknown field"));
    }

    #[test]
    fn validate_runtime_output_rejects_non_runtime_kind() {
        let error =
            validate_runtime_output(&valid_output("queue"), &intent(TransitionKind::Queue, None))
                .expect_err("non-runtime kinds should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error
            .message
            .contains("runtime output must use runtime intent kinds only"));
    }

    #[test]
    fn validate_runtime_output_rejects_empty_summary() {
        let error = validate_runtime_output(
            &valid_output_with_summary("complete", ""),
            &intent(TransitionKind::Complete, Some("done".to_owned())),
        )
        .expect_err("empty summary should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error.message.contains("patch.summary must be non-empty"));
    }

    #[test]
    fn validate_runtime_output_rejects_raw_output_drift_from_normalized_intent() {
        let error = validate_runtime_output(
            &valid_output_with_summary("complete", "summary from raw output"),
            &intent(TransitionKind::Complete, Some("done".to_owned())),
        )
        .expect_err("raw output that drifts from normalized intent should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error
            .message
            .contains("raw output does not match normalized transition intent"));
    }

    #[test]
    fn validate_runtime_output_rejects_non_uuid_ids() {
        let error = validate_runtime_output(
            &valid_output_with_ids("work-1", "agent-1", "lease-1", "complete"),
            &intent(TransitionKind::Complete, Some("done".to_owned())),
        )
        .expect_err("non-uuid ids should fail");

        assert_eq!(error.kind, RuntimeErrorKind::InvalidOutput);
        assert!(error.message.contains("must use uuid format"));
    }

    fn valid_output(kind: &str) -> String {
        valid_output_with_summary(kind, "summary")
    }

    fn valid_output_with_summary(kind: &str, summary: &str) -> String {
        valid_output_with_ids_and_summary(
            "11111111-1111-4111-8111-111111111111",
            "22222222-2222-4222-8222-222222222222",
            "33333333-3333-4333-8333-333333333333",
            kind,
            summary,
        )
    }

    fn valid_output_with_ids(work_id: &str, agent_id: &str, lease_id: &str, kind: &str) -> String {
        valid_output_with_ids_and_summary(work_id, agent_id, lease_id, kind, "summary")
    }

    fn valid_output_with_ids_and_summary(
        work_id: &str,
        agent_id: &str,
        lease_id: &str,
        kind: &str,
        summary: &str,
    ) -> String {
        format!(
            "{{\"work_id\":\"{work_id}\",\"agent_id\":\"{agent_id}\",\"lease_id\":\"{lease_id}\",\"expected_rev\":9,\"kind\":\"{kind}\",\"patch\":{{\"summary\":\"{summary}\",\"resolved_obligations\":[],\"declared_risks\":[]}},\"proof_hints\":[{{\"kind\":\"summary\",\"value\":\"summary\"}}]}}"
        )
    }

    fn valid_output_with_extra_field(kind: &str) -> String {
        format!(
            "{{\"work_id\":\"11111111-1111-4111-8111-111111111111\",\"agent_id\":\"22222222-2222-4222-8222-222222222222\",\"lease_id\":\"33333333-3333-4333-8333-333333333333\",\"expected_rev\":9,\"kind\":\"{kind}\",\"patch\":{{\"summary\":\"summary\",\"resolved_obligations\":[],\"declared_risks\":[]}},\"proof_hints\":[{{\"kind\":\"summary\",\"value\":\"summary\"}}],\"unexpected\":true}}"
        )
    }

    fn intent(kind: TransitionKind, note: Option<String>) -> TransitionIntent {
        TransitionIntent {
            work_id: WorkId::from("11111111-1111-4111-8111-111111111111"),
            agent_id: AgentId::from("22222222-2222-4222-8222-222222222222"),
            lease_id: LeaseId::from("33333333-3333-4333-8333-333333333333"),
            expected_rev: 9,
            kind,
            patch: WorkPatch {
                summary: "summary".to_owned(),
                resolved_obligations: Vec::new(),
                declared_risks: Vec::new(),
            },
            note,
            proof_hints: vec![ProofHint {
                kind: ProofHintKind::Summary,
                value: "summary".to_owned(),
            }],
        }
    }
}
