use crate::model::{
    CommandResult, ContractSet, ContractSetStatus, DecisionOutcome, EvidenceBundle, GateResult,
    GateSpec, LeaseEffect, PendingWake, PendingWakeEffect, ProofHintKind, ReasonCode,
    TransitionDecision, TransitionIntent, TransitionKind, TransitionRule, WorkLease, WorkSnapshot,
    WorkStatus,
};

use super::{
    apply::apply_snapshot_patch,
    claim::{actor_kind_for_intent, claim_lease, lease_is_stale},
    wake::pending_wake_effect_for,
};

pub(crate) fn decide_transition(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    pending_wake: Option<&PendingWake>,
    contract: &ContractSet,
    evidence: &EvidenceBundle,
    intent: &TransitionIntent,
) -> TransitionDecision {
    let mut reasons = Vec::new();
    let mut gate_results = Vec::new();
    let mut outcome = DecisionOutcome::Rejected;
    let mut lease_effect = LeaseEffect::None;
    let mut pending_wake_effect = pending_wake_effect_for(intent.kind, pending_wake);

    if intent.work_id != snapshot.work_id {
        reasons.push(ReasonCode::SchemaInvalid);
        return rejected_decision(
            outcome,
            reasons,
            gate_results,
            evidence,
            lease_effect,
            pending_wake_effect,
            "intent work_id does not match snapshot",
        );
    }

    if !contract_matches_snapshot(contract, snapshot) {
        reasons.push(ReasonCode::ContractDenied);
        return rejected_decision(
            outcome,
            reasons,
            gate_results,
            evidence,
            lease_effect,
            pending_wake_effect,
            "contract is not active for the snapshot",
        );
    }

    if intent.expected_rev != snapshot.rev {
        reasons.push(ReasonCode::RevConflict);
        return rejected_decision(
            DecisionOutcome::Conflict,
            reasons,
            gate_results,
            evidence,
            lease_effect,
            pending_wake_effect,
            "expected_rev does not match authoritative snapshot.rev",
        );
    }

    if requires_manual_note(intent.kind) && note_is_missing(intent.note.as_deref()) {
        reasons.push(ReasonCode::NoteMissing);
        return rejected_decision(
            outcome,
            reasons,
            gate_results,
            evidence,
            lease_effect,
            pending_wake_effect,
            "manual transition requires note",
        );
    }

    if lease_is_stale(snapshot, lease, intent) {
        reasons.push(ReasonCode::StaleLease);
        return rejected_decision(
            outcome,
            reasons,
            gate_results,
            evidence,
            lease_effect,
            pending_wake_effect,
            "lease is stale or held by another actor",
        );
    }

    if intent.kind == TransitionKind::Claim {
        if let Err(reason) = claim_lease(snapshot) {
            reasons.push(reason);
            return rejected_decision(
                DecisionOutcome::Conflict,
                reasons,
                gate_results,
                evidence,
                lease_effect,
                pending_wake_effect,
                "claim requires no open lease on the work",
            );
        }
    }

    let actor_kind = actor_kind_for_intent(intent.kind);
    let rule = match find_rule(contract, snapshot.status, intent.kind, actor_kind) {
        Some(rule) => rule,
        None => {
            reasons.push(ReasonCode::ContractDenied);
            return rejected_decision(
                outcome,
                reasons,
                gate_results,
                evidence,
                lease_effect,
                pending_wake_effect,
                "no matching transition rule for current state",
            );
        }
    };

    gate_results = evaluate_gates(rule, snapshot, lease, pending_wake, evidence, intent);
    if gate_results.iter().any(|result| !result.passed) {
        reasons.push(ReasonCode::GateFailed);
        return rejected_decision(
            outcome,
            reasons,
            gate_results,
            evidence,
            lease_effect,
            pending_wake_effect,
            "one or more contract gates failed",
        );
    }

    lease_effect = rule.lease_effect;
    pending_wake_effect = pending_wake_effect_for(intent.kind, pending_wake);
    outcome = if intent.kind == TransitionKind::OverrideComplete {
        DecisionOutcome::OverrideAccepted
    } else {
        DecisionOutcome::Accepted
    };

    TransitionDecision {
        outcome,
        reasons,
        next_snapshot: Some(apply_snapshot_patch(
            snapshot,
            intent,
            rule.to,
            lease_effect,
        )),
        lease_effect,
        pending_wake_effect,
        gate_results,
        evidence: evidence.clone(),
        summary: decision_summary(outcome, intent.kind, rule.to),
    }
}

pub(crate) fn command_gate_specs(
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    contract: &ContractSet,
    intent: &TransitionIntent,
) -> Vec<GateSpec> {
    if intent.work_id != snapshot.work_id {
        return Vec::new();
    }

    if !contract_matches_snapshot(contract, snapshot) {
        return Vec::new();
    }

    if intent.expected_rev != snapshot.rev {
        return Vec::new();
    }

    if requires_manual_note(intent.kind) && note_is_missing(intent.note.as_deref()) {
        return Vec::new();
    }

    if lease_is_stale(snapshot, lease, intent) {
        return Vec::new();
    }

    if intent.kind == TransitionKind::Claim && claim_lease(snapshot).is_err() {
        return Vec::new();
    }

    let actor_kind = actor_kind_for_intent(intent.kind);
    let Some(rule) = find_rule(contract, snapshot.status, intent.kind, actor_kind) else {
        return Vec::new();
    };

    rule.gates
        .iter()
        .filter_map(|gate| match gate {
            GateSpec::CommandSucceeds { .. } => Some(gate.clone()),
            _ => None,
        })
        .collect()
}

fn contract_matches_snapshot(contract: &ContractSet, snapshot: &WorkSnapshot) -> bool {
    contract.status == ContractSetStatus::Active
        && contract.company_id == snapshot.company_id
        && contract.contract_set_id == snapshot.contract_set_id
        && contract.revision == snapshot.contract_rev
}

fn find_rule(
    contract: &ContractSet,
    status: WorkStatus,
    kind: TransitionKind,
    actor_kind: crate::model::ActorKind,
) -> Option<&TransitionRule> {
    contract.rules.iter().find(|rule| {
        rule.kind == kind && rule.actor_kind == actor_kind && rule.from.contains(&status)
    })
}

fn evaluate_gates(
    rule: &TransitionRule,
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    pending_wake: Option<&PendingWake>,
    evidence: &EvidenceBundle,
    intent: &TransitionIntent,
) -> Vec<GateResult> {
    rule.gates
        .iter()
        .cloned()
        .map(|gate| evaluate_gate(gate, snapshot, lease, pending_wake, evidence, intent))
        .collect()
}

fn evaluate_gate(
    gate: GateSpec,
    snapshot: &WorkSnapshot,
    lease: Option<&WorkLease>,
    pending_wake: Option<&PendingWake>,
    evidence: &EvidenceBundle,
    intent: &TransitionIntent,
) -> GateResult {
    let (passed, detail) = match &gate {
        GateSpec::NoOpenLease => (
            snapshot.active_lease_id.is_none(),
            "work must not have an open lease".to_owned(),
        ),
        GateSpec::AgentIsRunnable => (
            evidence.observed_agent_status == Some(crate::model::AgentStatus::Active)
                && evidence.observed_agent_company_id.as_ref() == Some(&snapshot.company_id),
            "claiming agent must be active and belong to the same company".to_owned(),
        ),
        GateSpec::LeasePresent => (
            lease.is_some() && snapshot.active_lease_id.is_some(),
            "transition requires an active lease".to_owned(),
        ),
        GateSpec::LeaseHeldByActor => (
            lease.is_some_and(|current| current.agent_id == intent.agent_id)
                && snapshot
                    .active_lease_id
                    .as_ref()
                    .is_some_and(|lease_id| lease_id == &intent.lease_id),
            "lease must belong to the submitting actor".to_owned(),
        ),
        GateSpec::ExpectedRevMatchesSnapshot => (
            intent.expected_rev == snapshot.rev,
            "expected_rev must match snapshot.rev".to_owned(),
        ),
        GateSpec::SummaryPresent => (
            !intent.patch.summary.trim().is_empty()
                || intent
                    .proof_hints
                    .iter()
                    .any(|hint| hint.kind == ProofHintKind::Summary),
            "summary proof is required".to_owned(),
        ),
        GateSpec::ManualNotePresent => (
            !note_is_missing(intent.note.as_deref()),
            "manual note is required".to_owned(),
        ),
        GateSpec::ChangedFilesObserved => (
            !evidence.changed_files.is_empty(),
            "at least one changed file must be observed".to_owned(),
        ),
        GateSpec::AllRequiredObligationsResolved => {
            let unresolved = pending_wake
                .map(|wake| {
                    wake.obligation_json
                        .iter()
                        .filter(|obligation| {
                            !intent
                                .patch
                                .resolved_obligations
                                .iter()
                                .any(|item| item == *obligation)
                        })
                        .count()
                })
                .unwrap_or(0);
            (
                unresolved == 0,
                "all pending obligations must be resolved".to_owned(),
            )
        }
        GateSpec::CommandSucceeds {
            argv,
            allow_exit_codes,
            ..
        } => {
            let matched = evidence
                .command_results
                .iter()
                .find(|result| command_matches(result, argv));
            (
                matched.is_some_and(|result| allow_exit_codes.contains(&result.exit_code)),
                command_gate_detail(argv, matched),
            )
        }
    };

    GateResult {
        gate,
        passed,
        detail,
    }
}

fn command_matches(result: &CommandResult, argv: &[String]) -> bool {
    result.argv == argv
}

fn command_gate_detail(argv: &[String], result: Option<&CommandResult>) -> String {
    match result {
        Some(result) if result.failure_detail.is_none() => {
            format!("command {:?} exited {}", argv, result.exit_code)
        }
        Some(result) => format!(
            "command {:?} exited {}: {}",
            argv,
            result.exit_code,
            result.failure_detail.as_deref().unwrap_or("command failed")
        ),
        None => format!("command {:?} evidence missing", argv),
    }
}

fn requires_manual_note(kind: TransitionKind) -> bool {
    matches!(
        kind,
        TransitionKind::Block | TransitionKind::OverrideComplete
    )
}

fn note_is_missing(note: Option<&str>) -> bool {
    note.is_none_or(|value| value.trim().is_empty())
}

fn rejected_decision(
    outcome: DecisionOutcome,
    reasons: Vec<ReasonCode>,
    gate_results: Vec<GateResult>,
    evidence: &EvidenceBundle,
    lease_effect: LeaseEffect,
    pending_wake_effect: PendingWakeEffect,
    summary: &str,
) -> TransitionDecision {
    TransitionDecision {
        outcome,
        reasons,
        next_snapshot: None,
        lease_effect,
        pending_wake_effect,
        gate_results,
        evidence: evidence.clone(),
        summary: summary.to_owned(),
    }
}

fn decision_summary(
    outcome: DecisionOutcome,
    kind: TransitionKind,
    next_status: WorkStatus,
) -> String {
    format!("{kind:?} {outcome:?} with next status {next_status:?}")
}
