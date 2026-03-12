use std::{error::Error, fmt, time::Duration};

use crate::model::{CommandResult, FileChange};

pub(crate) trait WorkspacePort {
    fn current_dir(&self) -> Result<String, WorkspaceError>;
    fn observe_changed_files(&self, cwd: &str, hinted_paths: &[String]) -> Vec<FileChange>;
    fn run_gate_command(&self, cwd: &str, argv: &[String], timeout: Duration) -> CommandResult;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceError {
    pub(crate) kind: WorkspaceErrorKind,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceErrorKind {
    Unavailable,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for WorkspaceError {}
