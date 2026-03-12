use std::time::SystemTime;

use crate::model::{PendingWake, PendingWakeEffect, TransitionKind, WorkId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WakeRunPlan {
    SkipBecauseOpenLease,
    RefreshExistingRun,
    QueueNewRun,
    SkipBecauseNoRunnableAgent,
}

pub(crate) fn merge_wake(
    existing: Option<&PendingWake>,
    incoming_reason: &str,
    incoming_obligations: &[String],
    merged_at: SystemTime,
    work_id: WorkId,
) -> PendingWake {
    let mut obligation_json = existing
        .map(|wake| wake.obligation_json.clone())
        .unwrap_or_default();
    obligation_json.extend(incoming_obligations.iter().cloned());

    PendingWake {
        work_id: existing.map(|wake| wake.work_id.clone()).unwrap_or(work_id),
        obligation_json,
        count: existing.map_or(1, |wake| wake.count.saturating_add(1)),
        latest_reason: incoming_reason.to_owned(),
        merged_at,
    }
}

pub(super) fn pending_wake_effect_for(
    kind: TransitionKind,
    pending_wake: Option<&PendingWake>,
) -> PendingWakeEffect {
    match kind {
        TransitionKind::Complete if pending_wake.is_some() => PendingWakeEffect::Clear,
        TransitionKind::Reopen if pending_wake.is_some() => PendingWakeEffect::Retain,
        TransitionKind::Queue
        | TransitionKind::Claim
        | TransitionKind::ProposeProgress
        | TransitionKind::TimeoutRequeue => PendingWakeEffect::Retain,
        _ => PendingWakeEffect::None,
    }
}

pub(crate) fn wake_run_plan(
    has_open_lease: bool,
    has_runnable_run: bool,
    has_runnable_agent: bool,
) -> WakeRunPlan {
    if has_open_lease {
        WakeRunPlan::SkipBecauseOpenLease
    } else if has_runnable_run {
        WakeRunPlan::RefreshExistingRun
    } else if has_runnable_agent {
        WakeRunPlan::QueueNewRun
    } else {
        WakeRunPlan::SkipBecauseNoRunnableAgent
    }
}
