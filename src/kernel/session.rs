use crate::model::TaskSession;

pub(crate) fn advance_session(
    existing: Option<&TaskSession>,
    candidate: TaskSession,
    invalid_session: bool,
) -> TaskSession {
    match existing {
        Some(current)
            if !invalid_session
                && current.agent_id == candidate.agent_id
                && current.work_id == candidate.work_id
                && current.cwd == candidate.cwd
                && current.runtime == candidate.runtime =>
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
