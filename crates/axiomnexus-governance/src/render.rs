use triad_core::{ApplyPatchReport, NextClaim, RunClaimReport, StatusReport, VerifyReport};

pub(crate) fn render_init() -> String {
    "axiomnexus governance initialized".to_string()
}

pub(crate) fn render_next(next: &NextClaim) -> String {
    format!(
        "{} | {:?} | {:?} | {}",
        next.claim_id, next.next_action, next.status, next.reason
    )
}

pub(crate) fn render_status(report: &StatusReport) -> String {
    let mut lines = vec![format!(
        "healthy={} needs_code={} needs_test={} needs_spec={} contradicted={} blocked={}",
        report.summary.healthy,
        report.summary.needs_code,
        report.summary.needs_test,
        report.summary.needs_spec,
        report.summary.contradicted,
        report.summary.blocked
    )];

    for claim in &report.claims {
        lines.push(format!(
            "{} | {:?} | rev={} | patch={}",
            claim.claim_id,
            claim.status,
            claim.revision,
            claim
                .pending_patch_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".to_string())
        ));
    }

    lines.join("\n")
}

pub(crate) fn render_work(report: &RunClaimReport) -> String {
    format!(
        "claim={} run={} needs_patch={} changed_paths={}",
        report.claim_id,
        report.run_id,
        report.needs_patch,
        if report.changed_paths.is_empty() {
            "-".to_string()
        } else {
            report.changed_paths.join(",")
        }
    )
}

pub(crate) fn render_verify(report: &VerifyReport) -> String {
    format!(
        "claim={} verdict={:?} status_after_verify={:?} evidence_ids={}",
        report.claim_id,
        report.verdict,
        report.status_after_verify,
        report
            .evidence_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(crate) fn render_accept(report: &ApplyPatchReport) -> String {
    format!(
        "patch={} claim={} applied={} revision={} followup={:?}",
        report.patch_id,
        report.claim_id,
        report.applied,
        report.new_revision,
        report.followup_action
    )
}
