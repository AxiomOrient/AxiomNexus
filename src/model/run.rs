use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl RunStatus {
    pub(crate) fn is_runnable(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

#[cfg(test)]
mod tests {
    use super::RunStatus;

    #[test]
    fn run_status_serde_and_runnable_contract_stay_fixed() {
        let encoded = serde_json::to_string(&RunStatus::TimedOut).expect("status should encode");
        let decoded: RunStatus =
            serde_json::from_str("\"completed\"").expect("status should decode");

        assert_eq!(encoded, "\"timed_out\"");
        assert_eq!(decoded, RunStatus::Completed);
        assert!(RunStatus::Queued.is_runnable());
        assert!(RunStatus::Running.is_runnable());
        assert!(!RunStatus::Completed.is_runnable());
        assert!(!RunStatus::Failed.is_runnable());
        assert!(!RunStatus::Cancelled.is_runnable());
        assert!(!RunStatus::TimedOut.is_runnable());
    }
}
