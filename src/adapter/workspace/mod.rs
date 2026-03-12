use std::{
    collections::BTreeSet,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::{
    model::{ChangeKind, CommandResult, FileChange},
    port::workspace::{WorkspaceError, WorkspaceErrorKind, WorkspacePort},
};

const COMMAND_OUTPUT_LIMIT: usize = 4096;
const COMMAND_TIMEOUT_EXIT_CODE: i32 = 124;
const COMMAND_FAILURE_EXIT_CODE: i32 = -1;
const GATE_COMMAND_ALLOWLIST: &[&[&str]] = &[
    &["cargo", "fmt", "--all", "--check"],
    &[
        "cargo",
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ],
    &["cargo", "test"],
    &["cargo", "--version"],
];

#[derive(Debug, Clone, Default)]
pub(crate) struct SystemWorkspace;

impl WorkspacePort for SystemWorkspace {
    fn current_dir(&self) -> Result<String, WorkspaceError> {
        std::env::current_dir()
            .map(|cwd| cwd.display().to_string())
            .map_err(|error| WorkspaceError {
                kind: WorkspaceErrorKind::Unavailable,
                message: format!("workspace could not resolve current cwd: {error}"),
            })
    }

    fn observe_changed_files(&self, cwd: &str, hinted_paths: &[String]) -> Vec<FileChange> {
        let hinted_paths = hinted_paths
            .iter()
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if hinted_paths.is_empty() {
            return Vec::new();
        }

        let output = match Command::new("git")
            .args(["status", "--short", "--untracked-files=all"])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
        {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };

        let mut observed = Vec::new();
        let mut seen = BTreeSet::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(change) = parse_changed_file(line, &hinted_paths) {
                if seen.insert(change.path.clone()) {
                    observed.push(change);
                }
            }
        }

        observed
    }

    fn run_gate_command(&self, cwd: &str, argv: &[String], timeout: Duration) -> CommandResult {
        if argv.is_empty() {
            return failed_command_result(argv, "command argv must not be empty");
        }

        if !command_is_allowed(argv) {
            return failed_command_result(argv, "command argv is not in the allowlist");
        }

        let mut child = match Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return failed_command_result(argv, &format!("command spawn failed: {error}"));
            }
        };

        let stdout = child
            .stdout
            .take()
            .expect("spawned command should expose stdout");
        let stderr = child
            .stderr
            .take()
            .expect("spawned command should expose stderr");
        let stdout_reader = thread::spawn(move || read_all(stdout));
        let stderr_reader = thread::spawn(move || read_all(stderr));

        let started_at = Instant::now();
        let mut exit_code = COMMAND_FAILURE_EXIT_CODE;
        let mut failure_detail = None;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_code = status.code().unwrap_or(COMMAND_FAILURE_EXIT_CODE);
                    if !status.success() {
                        failure_detail = Some(format!("command exited with code {exit_code}"));
                    }
                    break;
                }
                Ok(None) if started_at.elapsed() >= timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    exit_code = COMMAND_TIMEOUT_EXIT_CODE;
                    failure_detail =
                        Some(format!("command timed out after {}s", timeout.as_secs()));
                    break;
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    failure_detail = Some(format!("command wait failed: {error}"));
                    break;
                }
            }
        }

        CommandResult {
            argv: argv.to_vec(),
            exit_code,
            stdout: bound_output(&stdout_reader.join().unwrap_or_default()),
            stderr: bound_output(&stderr_reader.join().unwrap_or_default()),
            failure_detail,
        }
    }
}

fn parse_changed_file(line: &str, hinted_paths: &BTreeSet<String>) -> Option<FileChange> {
    if line.len() < 4 {
        return None;
    }

    let status = &line[..2];
    let raw_path = line[3..].trim();
    let path = raw_path.rsplit(" -> ").next().unwrap_or(raw_path).trim();
    if !hinted_paths.contains(path) {
        return None;
    }

    Some(FileChange {
        path: path.to_owned(),
        change_kind: change_kind_from_git_status(status),
    })
}

fn change_kind_from_git_status(status: &str) -> ChangeKind {
    if status.contains('D') {
        ChangeKind::Deleted
    } else if status.contains('?') || status.contains('A') {
        ChangeKind::Added
    } else {
        ChangeKind::Modified
    }
}

fn failed_command_result(argv: &[String], detail: &str) -> CommandResult {
    CommandResult {
        argv: argv.to_vec(),
        exit_code: COMMAND_FAILURE_EXIT_CODE,
        stdout: String::new(),
        stderr: String::new(),
        failure_detail: Some(detail.to_owned()),
    }
}

fn command_is_allowed(argv: &[String]) -> bool {
    GATE_COMMAND_ALLOWLIST
        .iter()
        .any(|allowed| argv.iter().map(String::as_str).eq(allowed.iter().copied()))
}

fn read_all<R: Read>(mut reader: R) -> Vec<u8> {
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer);
    buffer
}

fn bound_output(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= COMMAND_OUTPUT_LIMIT {
        return text.into_owned();
    }

    let mut bounded = text.chars().take(COMMAND_OUTPUT_LIMIT).collect::<String>();
    bounded.push_str("\n...[truncated]");
    bounded
}
