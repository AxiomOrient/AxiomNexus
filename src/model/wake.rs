use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Timestamp, WorkId};

pub(crate) type ObligationSet = BTreeSet<String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PendingWake {
    pub(crate) work_id: WorkId,
    pub(crate) obligation_json: ObligationSet,
    pub(crate) count: u32,
    pub(crate) latest_reason: String,
    pub(crate) merged_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use serde_json::json;

    use super::PendingWake;
    use crate::model::WorkId;

    #[test]
    fn pending_wake_roundtrip_keeps_count_and_obligation_set_separate() {
        let wake = PendingWake {
            work_id: WorkId::from("work-1"),
            obligation_json: ["cargo fmt", "cargo test"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            count: 3,
            latest_reason: "gate failed".to_owned(),
            merged_at: SystemTime::UNIX_EPOCH + Duration::from_secs(42),
        };

        let encoded = serde_json::to_value(&wake).expect("pending wake should encode");
        assert_eq!(encoded["count"], json!(3));
        assert_eq!(
            encoded["obligation_json"],
            json!(["cargo fmt", "cargo test"])
        );

        let decoded: PendingWake =
            serde_json::from_value(encoded).expect("pending wake should decode");
        assert_eq!(decoded, wake);
        assert_eq!(decoded.count, 3);
        assert_eq!(decoded.obligation_json.len(), 2);
    }
}
