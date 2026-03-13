use crate::model::{
    workspace_fingerprint, AgentId, RuntimeKind, SessionInvalidationReason, TaskSession, WorkId,
};

pub(crate) fn session_invalidation_reason(
    existing: &TaskSession,
    agent_id: &AgentId,
    work_id: &WorkId,
    cwd: &str,
    runtime: RuntimeKind,
) -> Option<SessionInvalidationReason> {
    if existing.agent_id != *agent_id {
        return Some(SessionInvalidationReason::Agent);
    }
    if existing.work_id != *work_id {
        return Some(SessionInvalidationReason::Work);
    }
    if existing.workspace_fingerprint != workspace_fingerprint(cwd) {
        return Some(SessionInvalidationReason::Workspace);
    }
    if existing.runtime != runtime {
        return Some(SessionInvalidationReason::Runtime);
    }

    None
}

pub(crate) fn advance_session(
    existing: Option<&TaskSession>,
    candidate: TaskSession,
    invalidation_reason: Option<SessionInvalidationReason>,
) -> TaskSession {
    match existing {
        Some(current)
            if invalidation_reason.is_none()
                && session_invalidation_reason(
                    current,
                    &candidate.agent_id,
                    &candidate.work_id,
                    &candidate.cwd,
                    candidate.runtime,
                )
                .is_none() =>
        {
            let mut resumed = current.clone();
            resumed.contract_rev = candidate.contract_rev;
            resumed.last_record_id = candidate.last_record_id;
            resumed.last_decision_summary = candidate.last_decision_summary;
            resumed.last_gate_summary = candidate.last_gate_summary;
            resumed.updated_at = candidate.updated_at;
            resumed
        }
        _ => candidate,
    }
}
