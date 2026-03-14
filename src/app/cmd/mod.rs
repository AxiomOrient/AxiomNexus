#![allow(dead_code)]

use std::time::Duration;

pub(crate) const DECISION_PATH: &str = "kernel::decide_transition -> StorePort.commit_decision";
pub(crate) const WAKE_QUEUE_POLICY: &str = "pending_wakes merge or create; no queue fan-out";
pub(crate) const COMMENTS_TABLE: &str = "work_comments";
pub(crate) const RUNTIME_RESUME_POLICY: &str =
    "resume first; clear invalid session; retry fresh once";
pub(crate) const SCHEDULER_PICK_POLICY: &str = "oldest queued run first";
pub(crate) const AGENT_STATUS_POLICY: &str =
    "paused agents reject new claim or run start; terminated agents cannot resume";
pub(crate) const RUN_REAPER_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) mod activate_contract;
pub(crate) mod append_comment;
pub(crate) mod claim_work;
pub(crate) mod create_agent;
pub(crate) mod create_company;
pub(crate) mod create_contract_draft;
pub(crate) mod create_work;
pub(crate) mod resume_session;
pub(crate) mod run_scheduler;
pub(crate) mod run_turn_once;
pub(crate) mod set_agent_status;
pub(crate) mod submit_intent;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod update_work;
pub(crate) mod wake_work;
