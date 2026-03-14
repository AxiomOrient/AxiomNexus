use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use coclai::runtime::{Client, ClientError, PromptRunError, RpcError, SessionConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    model::{
        ChangeKind, CommandResult, ConsumptionUsage, FileChange, ProofHintKind,
        SessionInvalidationReason, TransitionIntent,
    },
    port::runtime::{
        ExecuteTurnOutcome, ExecuteTurnReq, PromptEnvelopeInput, RuntimeError, RuntimeErrorKind,
        RuntimeHandle, RuntimeObservations, RuntimePort, RuntimeResult,
    },
};

use super::assets::RuntimeAssets;
use super::contract::{output_rule_line, validate_runtime_output, INVALID_OUTPUT_REPAIR_BUDGET};
use crate::port::store::SessionKey;

pub(crate) struct CoclaiRuntime {
    assets: RuntimeAssets,
    backend: RuntimeBackend,
}

enum RuntimeBackend {
    Live(Box<LiveRuntime>),
    Scripted(RefCell<VecDeque<ScriptedReply>>),
}

struct LiveRuntime {
    schema: Value,
    client: RefCell<Option<LiveClient>>,
    pending_turns: RefCell<BTreeMap<String, PendingTurn>>,
}

struct LiveClient {
    runtime: tokio::runtime::Runtime,
    client: Client,
}

const COMMAND_OUTPUT_LIMIT: usize = 4096;
const COMMAND_TIMEOUT_EXIT_CODE: i32 = 124;
const COMMAND_FAILURE_EXIT_CODE: i32 = -1;
const SCRIPTED_REPLIES_PATH_ENV: &str = "AXIOMNEXUS_COCLAI_SCRIPT_PATH";
const ALLOW_SCRIPTED_RUNTIME_ENV: &str = "AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME";
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

#[derive(Clone)]
struct PendingTurn {
    session_key: SessionKey,
    cwd: String,
    prompt_envelope: String,
    attempt_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartTurn {
    session_key: SessionKey,
    cwd: String,
    prompt_envelope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResumeTurn {
    session_key: SessionKey,
    runtime_session_id: String,
    cwd: String,
    prompt_envelope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ScriptedReply {
    pub(crate) handle: RuntimeHandle,
    pub(crate) raw_output: String,
    pub(crate) intent: TransitionIntent,
    pub(crate) usage: ConsumptionUsage,
    pub(crate) invalid_session: bool,
}

impl CoclaiRuntime {
    pub(crate) fn from_repo_root(repo_root: &Path) -> Result<Self, RuntimeError> {
        let assets =
            RuntimeAssets::load_from_repo_root(repo_root).map_err(|error| RuntimeError {
                kind: RuntimeErrorKind::Unavailable,
                message: format!("failed to load coclai assets: {error}"),
            })?;
        let backend = match scripted_replies_path(repo_root) {
            Some(path) => {
                RuntimeBackend::Scripted(RefCell::new(load_scripted_replies(&path)?.into()))
            }
            None => RuntimeBackend::Live(Box::new(LiveRuntime::new(
                &assets.transition_intent_schema,
            )?)),
        };
        Ok(Self { assets, backend })
    }

    #[cfg(test)]
    pub(crate) fn with_scripted_replies(
        assets: RuntimeAssets,
        replies: Vec<ScriptedReply>,
    ) -> Self {
        Self {
            assets,
            backend: RuntimeBackend::Scripted(RefCell::new(replies.into())),
        }
    }

    pub(crate) fn build_prompt_envelope(&self, input: &PromptEnvelopeInput) -> String {
        let unresolved = if input.unresolved_obligations.is_empty() {
            "- none".to_owned()
        } else {
            input
                .unresolved_obligations
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let last_gate_summary = input.last_gate_summary.as_deref().unwrap_or("none");
        let last_decision_summary = input.last_decision_summary.as_deref().unwrap_or("none");

        format!(
            concat!(
                "{agents}\n\n",
                "{skill}\n\n",
                "Current work snapshot:\n",
                "- work_id: {work_id}\n",
                "- title: {title}\n",
                "- status: {status:?}\n",
                "- rev: {rev}\n\n",
                "Unresolved obligations:\n",
                "{unresolved}\n\n",
                "Pinned contract revision summary:\n",
                "{contract_summary}\n\n",
                "Last rejected gate summary:\n",
                "{last_gate_summary}\n\n",
                "Last accepted decision summary:\n",
                "{last_decision_summary}\n\n",
                "{output_rule}"
            ),
            agents = self.assets.agents_md,
            skill = self.assets.transition_executor_skill,
            work_id = input.snapshot.work_id,
            title = input.snapshot.title,
            status = input.snapshot.status,
            rev = input.snapshot.rev,
            unresolved = unresolved,
            contract_summary = input.contract_summary,
            last_gate_summary = last_gate_summary,
            last_decision_summary = last_decision_summary,
            output_rule = output_rule_line(),
        )
    }

    fn execute_turn_inner(&self, req: ExecuteTurnReq) -> Result<ExecuteTurnOutcome, RuntimeError> {
        let prompt_envelope = self.build_prompt_envelope(&req.prompt_input);
        let mut resumed = false;
        let mut repair_count = 0;
        let mut session_reset_reason = None;
        let start_req = StartTurn {
            session_key: req.session_key.clone(),
            cwd: req.cwd.clone(),
            prompt_envelope: prompt_envelope.clone(),
        };
        let mut handle = if let Some(session) = req.existing_session.as_ref() {
            resumed = true;
            self.resume_turn(ResumeTurn {
                session_key: req.session_key.clone(),
                runtime_session_id: session.runtime_session_id.clone(),
                cwd: req.cwd.clone(),
                prompt_envelope: prompt_envelope.clone(),
            })?
        } else {
            self.start_turn(start_req.clone())?
        };

        loop {
            let result = self.result_turn(handle.clone())?;
            if result.invalid_session && resumed {
                resumed = false;
                session_reset_reason = Some(SessionInvalidationReason::Runtime);
                handle = self.start_turn(start_req.clone())?;
                continue;
            }

            if validate_runtime_output(&result.raw_output, &result.intent).is_ok() {
                let observations = match &self.backend {
                    RuntimeBackend::Live(_) => {
                        collect_observations(&req.cwd, &result.intent, &req.gate_plan)
                    }
                    RuntimeBackend::Scripted(_) => {
                        scripted_observations(&result.intent, &req.gate_plan)
                    }
                };
                return Ok(ExecuteTurnOutcome {
                    handle,
                    result,
                    resumed,
                    repair_count,
                    session_reset_reason,
                    prompt_envelope,
                    observations,
                });
            }

            if repair_count >= INVALID_OUTPUT_REPAIR_BUDGET {
                return Err(RuntimeError {
                    kind: RuntimeErrorKind::InvalidOutput,
                    message: "repair retry budget exhausted".to_owned(),
                });
            }

            repair_count += 1;
        }
    }

    fn start_turn(&self, req: StartTurn) -> Result<RuntimeHandle, RuntimeError> {
        match &self.backend {
            RuntimeBackend::Live(live) => live.start(req),
            RuntimeBackend::Scripted(replies) => replies
                .borrow()
                .front()
                .map(|reply| reply.handle.clone())
                .ok_or_else(|| RuntimeError {
                    kind: RuntimeErrorKind::Unavailable,
                    message: "no scripted coclai start reply available".to_owned(),
                }),
        }
    }

    fn resume_turn(&self, req: ResumeTurn) -> Result<RuntimeHandle, RuntimeError> {
        if req.runtime_session_id.trim().is_empty() {
            return Err(RuntimeError {
                kind: RuntimeErrorKind::InvalidSession,
                message: "runtime session id is required to resume".to_owned(),
            });
        }

        match &self.backend {
            RuntimeBackend::Live(live) => live.resume(req),
            RuntimeBackend::Scripted(replies) => replies
                .borrow()
                .front()
                .map(|reply| reply.handle.clone())
                .ok_or_else(|| RuntimeError {
                    kind: RuntimeErrorKind::Unavailable,
                    message: "no scripted coclai resume reply available".to_owned(),
                }),
        }
    }

    fn result_turn(&self, handle: RuntimeHandle) -> Result<RuntimeResult, RuntimeError> {
        match &self.backend {
            RuntimeBackend::Live(live) => live.result(handle),
            RuntimeBackend::Scripted(replies) => {
                let reply = replies
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| RuntimeError {
                        kind: RuntimeErrorKind::Unavailable,
                        message: "no scripted coclai result available".to_owned(),
                    })?;

                Ok(RuntimeResult {
                    intent: reply.intent,
                    raw_output: reply.raw_output,
                    usage: reply.usage,
                    invalid_session: reply.invalid_session,
                })
            }
        }
    }
}

fn scripted_observations(
    intent: &TransitionIntent,
    gate_plan: &[crate::port::runtime::GateCommandSpec],
) -> RuntimeObservations {
    RuntimeObservations {
        changed_files: intent
            .proof_hints
            .iter()
            .filter(|hint| hint.kind == ProofHintKind::File)
            .map(|hint| FileChange {
                path: hint.value.clone(),
                change_kind: ChangeKind::Modified,
            })
            .collect(),
        command_results: gate_plan
            .iter()
            .filter(|spec| spec.applies_to_kind == intent.kind)
            .map(|spec| CommandResult {
                argv: spec.argv.clone(),
                exit_code: 0,
                stdout: "ok".to_owned(),
                stderr: String::new(),
                failure_detail: None,
            })
            .collect(),
        artifact_refs: Vec::new(),
        notes: None,
    }
}

fn scripted_replies_path(repo_root: &Path) -> Option<PathBuf> {
    if !allow_scripted_runtime() {
        return None;
    }

    env::var_os(SCRIPTED_REPLIES_PATH_ENV).map(|value| {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            repo_root.join(path)
        }
    })
}

fn allow_scripted_runtime() -> bool {
    matches!(
        env::var(ALLOW_SCRIPTED_RUNTIME_ENV).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn load_scripted_replies(path: &Path) -> Result<Vec<ScriptedReply>, RuntimeError> {
    let raw = fs::read_to_string(path).map_err(|error| RuntimeError {
        kind: RuntimeErrorKind::Unavailable,
        message: format!(
            "failed to read scripted coclai replies from {}: {error}",
            path.display()
        ),
    })?;
    let replies =
        serde_json::from_str::<Vec<ScriptedReply>>(&raw).map_err(|error| RuntimeError {
            kind: RuntimeErrorKind::InvalidOutput,
            message: format!(
                "failed to parse scripted coclai replies from {}: {error}",
                path.display()
            ),
        })?;
    if replies.is_empty() {
        return Err(RuntimeError {
            kind: RuntimeErrorKind::Unavailable,
            message: format!(
                "scripted coclai replies file {} must contain at least one reply",
                path.display()
            ),
        });
    }
    Ok(replies)
}

fn collect_observations(
    cwd: &str,
    intent: &TransitionIntent,
    gate_plan: &[crate::port::runtime::GateCommandSpec],
) -> RuntimeObservations {
    let hinted_paths = intent
        .proof_hints
        .iter()
        .filter(|hint| hint.kind == ProofHintKind::File)
        .map(|hint| hint.value.trim())
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let command_results = gate_plan
        .iter()
        .filter(|spec| spec.applies_to_kind == intent.kind)
        .map(|spec| run_gate_command(cwd, &spec.argv, Duration::from_secs(spec.timeout_sec)))
        .collect();

    RuntimeObservations {
        changed_files: observe_changed_files(cwd, &hinted_paths),
        command_results,
        artifact_refs: Vec::new(),
        notes: None,
    }
}

fn observe_changed_files(cwd: &str, hinted_paths: &[String]) -> Vec<FileChange> {
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

fn run_gate_command(cwd: &str, argv: &[String], timeout: Duration) -> CommandResult {
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
                failure_detail = Some(format!("command timed out after {}s", timeout.as_secs()));
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
    let bounded = if bytes.len() > COMMAND_OUTPUT_LIMIT {
        &bytes[..COMMAND_OUTPUT_LIMIT]
    } else {
        bytes
    };
    String::from_utf8_lossy(bounded).into_owned()
}

impl RuntimePort for CoclaiRuntime {
    fn execute_turn(&self, req: ExecuteTurnReq) -> Result<ExecuteTurnOutcome, RuntimeError> {
        self.execute_turn_inner(req)
    }
}

impl LiveRuntime {
    fn new(schema_source: &str) -> Result<Self, RuntimeError> {
        let schema = serde_json::from_str(schema_source).map_err(|error| RuntimeError {
            kind: RuntimeErrorKind::Unavailable,
            message: format!("failed to parse transition intent schema: {error}"),
        })?;

        Ok(Self {
            schema,
            client: RefCell::new(None),
            pending_turns: RefCell::new(BTreeMap::new()),
        })
    }

    fn start(&self, req: StartTurn) -> Result<RuntimeHandle, RuntimeError> {
        let handle = self.with_client(|live| {
            let config = session_config(&req.cwd, self.schema.clone());
            let session = live
                .runtime
                .block_on(live.client.start_session(config))
                .map_err(map_prompt_error)?;

            Ok(RuntimeHandle {
                runtime_session_id: session.thread_id.clone(),
            })
        })?;
        self.pending_turns.borrow_mut().insert(
            handle.runtime_session_id.clone(),
            PendingTurn {
                session_key: req.session_key,
                cwd: req.cwd,
                prompt_envelope: req.prompt_envelope,
                attempt_count: 0,
            },
        );
        Ok(handle)
    }

    fn resume(&self, req: ResumeTurn) -> Result<RuntimeHandle, RuntimeError> {
        let handle = RuntimeHandle {
            runtime_session_id: req.runtime_session_id.clone(),
        };
        self.pending_turns.borrow_mut().insert(
            handle.runtime_session_id.clone(),
            PendingTurn {
                session_key: req.session_key,
                cwd: req.cwd,
                prompt_envelope: req.prompt_envelope,
                attempt_count: 0,
            },
        );
        Ok(handle)
    }

    fn result(&self, handle: RuntimeHandle) -> Result<RuntimeResult, RuntimeError> {
        let (session_key, cwd, prompt) = {
            let mut pending_turns = self.pending_turns.borrow_mut();
            let pending = pending_turns
                .get_mut(handle.runtime_session_id.as_str())
                .ok_or_else(|| RuntimeError {
                    kind: RuntimeErrorKind::Unavailable,
                    message: format!(
                        "no pending coclai turn for runtime session {}",
                        handle.runtime_session_id
                    ),
                })?;
            let prompt = if pending.attempt_count == 0 {
                pending.prompt_envelope.clone()
            } else {
                repair_prompt(&pending.prompt_envelope)
            };
            pending.attempt_count += 1;
            (pending.session_key.clone(), pending.cwd.clone(), prompt)
        };

        let config = session_config(&cwd, self.schema.clone());
        let prompt_result = self.with_client(|live| {
            let session = match live.runtime.block_on(
                live.client
                    .resume_session(&handle.runtime_session_id, config),
            ) {
                Ok(session) => session,
                Err(error) if is_invalid_session_error(&error) => {
                    return Ok(LiveTurnResult::InvalidSession);
                }
                Err(error) => return Err(map_prompt_error(error)),
            };

            let started_at = Instant::now();
            match live.runtime.block_on(session.ask(prompt)) {
                Ok(result) => Ok(LiveTurnResult::Completed {
                    raw_output: result.assistant_text,
                    usage: live_usage(started_at.elapsed()),
                }),
                Err(error) if is_invalid_session_error(&error) => {
                    Ok(LiveTurnResult::InvalidSession)
                }
                Err(error) => Err(map_prompt_error(error)),
            }
        })?;

        match prompt_result {
            LiveTurnResult::InvalidSession => Ok(RuntimeResult {
                intent: placeholder_intent(&session_key),
                raw_output: String::new(),
                usage: ConsumptionUsage::default(),
                invalid_session: true,
            }),
            LiveTurnResult::Completed { raw_output, usage } => {
                let intent =
                    serde_json::from_str::<TransitionIntent>(&raw_output).map_err(|error| {
                        RuntimeError {
                            kind: RuntimeErrorKind::InvalidOutput,
                            message: format!(
                                "failed to parse coclai runtime output as TransitionIntent: {error}"
                            ),
                        }
                    })?;
                Ok(RuntimeResult {
                    intent,
                    raw_output,
                    usage,
                    invalid_session: false,
                })
            }
        }
    }

    fn with_client<T>(
        &self,
        f: impl FnOnce(&mut LiveClient) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let mut client = self.client.borrow_mut();
        if client.is_none() {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| RuntimeError {
                    kind: RuntimeErrorKind::Unavailable,
                    message: format!("failed to build coclai runtime: {error}"),
                })?;
            let connected = runtime
                .block_on(Client::connect_default())
                .map_err(map_client_error)?;
            *client = Some(LiveClient {
                runtime,
                client: connected,
            });
        }

        f(client.as_mut().expect("live client should be initialized"))
    }
}

enum LiveTurnResult {
    InvalidSession,
    Completed {
        raw_output: String,
        usage: ConsumptionUsage,
    },
}

fn session_config(cwd: &str, schema: Value) -> SessionConfig {
    SessionConfig::new(cwd)
        .with_timeout(Duration::from_secs(120))
        .with_output_schema(schema)
}

fn repair_prompt(prompt_envelope: &str) -> String {
    format!(
        "{prompt_envelope}\n\nPrevious output violated the AxiomNexus TransitionIntent contract. Return only corrected JSON."
    )
}

fn placeholder_intent(session_key: &SessionKey) -> TransitionIntent {
    TransitionIntent {
        work_id: session_key.work_id.clone(),
        agent_id: session_key.agent_id.clone(),
        lease_id: crate::model::LeaseId::from("00000000-0000-4000-8000-000000000000"),
        expected_rev: 0,
        kind: crate::model::TransitionKind::ProposeProgress,
        patch: crate::model::WorkPatch {
            summary: "invalid session".to_owned(),
            resolved_obligations: Vec::new(),
            declared_risks: Vec::new(),
        },
        note: None,
        proof_hints: vec![crate::model::ProofHint {
            kind: crate::model::ProofHintKind::Summary,
            value: "invalid session".to_owned(),
        }],
    }
}

fn live_usage(elapsed: Duration) -> ConsumptionUsage {
    ConsumptionUsage {
        input_tokens: 0,
        output_tokens: 0,
        run_seconds: elapsed.as_secs(),
        estimated_cost_cents: None,
    }
}

fn map_client_error(error: ClientError) -> RuntimeError {
    RuntimeError {
        kind: RuntimeErrorKind::Unavailable,
        message: format!("failed to connect coclai client: {error}"),
    }
}

fn map_prompt_error(error: PromptRunError) -> RuntimeError {
    match error {
        PromptRunError::Rpc(rpc_error) => map_rpc_error(rpc_error),
        PromptRunError::Runtime(runtime_error) => RuntimeError {
            kind: RuntimeErrorKind::Transport,
            message: format!("coclai runtime transport error: {runtime_error}"),
        },
        PromptRunError::TurnFailedWithContext(context) => RuntimeError {
            kind: RuntimeErrorKind::Transport,
            message: format!("coclai turn failed: {context}"),
        },
        PromptRunError::TurnFailed => RuntimeError {
            kind: RuntimeErrorKind::Transport,
            message: "coclai turn failed".to_owned(),
        },
        PromptRunError::TurnInterrupted => RuntimeError {
            kind: RuntimeErrorKind::Transport,
            message: "coclai turn interrupted".to_owned(),
        },
        PromptRunError::Timeout(duration) => RuntimeError {
            kind: RuntimeErrorKind::Transport,
            message: format!("coclai turn timed out after {duration:?}"),
        },
        PromptRunError::TurnCompletedWithoutAssistantText(context) => RuntimeError {
            kind: RuntimeErrorKind::InvalidOutput,
            message: format!("coclai turn completed without assistant text: {context}"),
        },
        PromptRunError::EmptyAssistantText => RuntimeError {
            kind: RuntimeErrorKind::InvalidOutput,
            message: "coclai assistant text was empty".to_owned(),
        },
        PromptRunError::AttachmentNotFound(path) => RuntimeError {
            kind: RuntimeErrorKind::Unavailable,
            message: format!("coclai attachment missing: {path}"),
        },
        PromptRunError::BlockedByHook {
            hook_name,
            phase,
            message,
        } => RuntimeError {
            kind: RuntimeErrorKind::Unavailable,
            message: format!("coclai blocked by hook {hook_name} at {phase:?}: {message}"),
        },
    }
}

fn map_rpc_error(error: RpcError) -> RuntimeError {
    let kind = if is_invalid_session_rpc(&error) {
        RuntimeErrorKind::InvalidSession
    } else {
        RuntimeErrorKind::Transport
    };
    RuntimeError {
        kind,
        message: format!("coclai rpc error: {error}"),
    }
}

fn is_invalid_session_error(error: &PromptRunError) -> bool {
    match error {
        PromptRunError::Rpc(rpc_error) => is_invalid_session_rpc(rpc_error),
        _ => false,
    }
}

fn is_invalid_session_rpc(error: &RpcError) -> bool {
    matches!(error, RpcError::InvalidRequest(_))
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::Path, process::Command, sync::Mutex, time::SystemTime};

    use crate::{
        adapter::coclai::assets::{
            AGENTS_ASSET_PATH, EXECUTE_TURN_OUTPUT_SCHEMA_PATH, TRANSITION_EXECUTOR_SKILL_PATH,
            TRANSITION_INTENT_SCHEMA_PATH,
        },
        model::{
            workspace_fingerprint, AgentId, CompanyId, ConsumptionUsage, ContractSetId, LeaseId,
            Priority, RuntimeKind, SessionId, SessionInvalidationReason, TaskSession,
            TransitionIntent, TransitionKind, WorkId, WorkKind, WorkPatch, WorkSnapshot,
            WorkStatus,
        },
        port::{
            runtime::{ExecuteTurnReq, GateCommandSpec, PromptEnvelopeInput, RuntimePort},
            store::SessionKey,
        },
    };

    use super::{
        CoclaiRuntime, RuntimeBackend, ScriptedReply, ALLOW_SCRIPTED_RUNTIME_ENV,
        SCRIPTED_REPLIES_PATH_ENV,
    };
    use crate::adapter::coclai::assets::RuntimeAssets;

    const WORK_ID: &str = "11111111-1111-4111-8111-111111111111";
    const AGENT_ID: &str = "22222222-2222-4222-8222-222222222222";
    const LEASE_ID: &str = "33333333-3333-4333-8333-333333333333";
    static SCRIPTED_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn from_repo_root_loads_canonical_runtime_assets() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let runtime = CoclaiRuntime::from_repo_root(repo_root).expect("runtime assets should load");

        assert_eq!(
            runtime.assets.agents_md,
            fs::read_to_string(repo_root.join(AGENTS_ASSET_PATH))
                .expect("agents asset should load")
        );
        assert_eq!(
            runtime.assets.transition_executor_skill,
            fs::read_to_string(repo_root.join(TRANSITION_EXECUTOR_SKILL_PATH))
                .expect("skill asset should load")
        );
        assert_eq!(
            runtime.assets.transition_intent_schema,
            fs::read_to_string(repo_root.join(TRANSITION_INTENT_SCHEMA_PATH))
                .expect("schema asset should load")
        );
        assert_eq!(
            runtime.assets.execute_turn_output_schema,
            fs::read_to_string(repo_root.join(EXECUTE_TURN_OUTPUT_SCHEMA_PATH))
                .expect("execute turn schema asset should load")
        );
    }

    #[test]
    fn from_repo_root_ignores_scripted_runtime_without_explicit_allow_flag() {
        let _guard = SCRIPTED_ENV_LOCK.lock().expect("env lock should work");
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let script_path = repo_root.join("samples/transition-intent.schema.json");
        let previous_script = env::var_os(SCRIPTED_REPLIES_PATH_ENV);
        let previous_allow = env::var_os(ALLOW_SCRIPTED_RUNTIME_ENV);

        env::set_var(SCRIPTED_REPLIES_PATH_ENV, &script_path);
        env::remove_var(ALLOW_SCRIPTED_RUNTIME_ENV);

        let runtime = CoclaiRuntime::from_repo_root(repo_root).expect("runtime should still load");

        assert!(matches!(runtime.backend, RuntimeBackend::Live(_)));

        restore_env(SCRIPTED_REPLIES_PATH_ENV, previous_script);
        restore_env(ALLOW_SCRIPTED_RUNTIME_ENV, previous_allow);
    }

    #[test]
    fn from_repo_root_uses_scripted_runtime_only_with_explicit_allow_flag() {
        let _guard = SCRIPTED_ENV_LOCK.lock().expect("env lock should work");
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let script_path = temp_scripted_reply_file();
        let previous_script = env::var_os(SCRIPTED_REPLIES_PATH_ENV);
        let previous_allow = env::var_os(ALLOW_SCRIPTED_RUNTIME_ENV);

        env::set_var(SCRIPTED_REPLIES_PATH_ENV, &script_path);
        env::set_var(ALLOW_SCRIPTED_RUNTIME_ENV, "1");

        let runtime =
            CoclaiRuntime::from_repo_root(repo_root).expect("scripted runtime should load");

        assert!(matches!(runtime.backend, RuntimeBackend::Scripted(_)));

        restore_env(SCRIPTED_REPLIES_PATH_ENV, previous_script);
        restore_env(ALLOW_SCRIPTED_RUNTIME_ENV, previous_allow);
        let _ = fs::remove_file(script_path);
    }

    #[test]
    fn prompt_envelope_contains_contract_and_output_rule() {
        let runtime = CoclaiRuntime::from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("runtime assets should load");
        let envelope = runtime.build_prompt_envelope(&prompt_input());

        assert!(envelope.contains("Current work snapshot"));
        assert!(envelope.contains("Unresolved obligations"));
        assert!(envelope.contains("Pinned contract revision summary"));
        assert!(envelope.contains("Last rejected gate summary"));
        assert!(envelope.contains("Last accepted decision summary"));
        assert!(envelope.contains("samples/transition-intent.schema.json"));
    }

    #[test]
    fn execute_turn_prefers_resume_and_falls_back_once_on_invalid_session() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                ScriptedReply {
                    handle: handle("runtime-old"),
                    raw_output: valid_output("complete", Some("fixed"), true),
                    intent: intent(TransitionKind::Complete, Some("fixed".to_owned())),
                    usage: usage(),
                    invalid_session: true,
                },
                ScriptedReply {
                    handle: handle("runtime-new"),
                    raw_output: valid_output("complete", Some("fixed"), true),
                    intent: intent(TransitionKind::Complete, Some("fixed".to_owned())),
                    usage: usage(),
                    invalid_session: false,
                },
            ],
        );

        let outcome = runtime
            .execute_turn(execute_turn_req(Some(session("/repo"))))
            .expect("fallback after invalid session should succeed");

        assert!(!outcome.resumed);
        assert_eq!(outcome.repair_count, 0);
        assert_eq!(
            outcome.session_reset_reason,
            Some(SessionInvalidationReason::Runtime)
        );
        assert_eq!(outcome.result.intent.kind, TransitionKind::Complete);
    }

    #[test]
    fn execute_turn_retries_invalid_output_once_then_fails() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                scripted_invalid_reply("runtime-1"),
                scripted_invalid_reply("runtime-1"),
            ],
        );

        let error = runtime
            .execute_turn(execute_turn_req(None))
            .expect_err("second invalid output should exhaust repair budget");

        assert_eq!(
            error.kind,
            crate::port::runtime::RuntimeErrorKind::InvalidOutput
        );
        assert!(error.message.contains("repair retry budget exhausted"));
    }

    #[test]
    fn execute_turn_accepts_valid_output_after_single_repair_retry() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                scripted_invalid_reply("runtime-1"),
                ScriptedReply {
                    handle: handle("runtime-1"),
                    raw_output: valid_output("complete", Some("fixed"), true),
                    intent: intent(TransitionKind::Complete, Some("fixed".to_owned())),
                    usage: usage(),
                    invalid_session: false,
                },
            ],
        );

        let outcome = runtime
            .execute_turn(execute_turn_req(None))
            .expect("single repair retry should succeed");

        assert_eq!(outcome.repair_count, 1);
        assert_eq!(outcome.result.intent.kind, TransitionKind::Complete);
    }

    #[test]
    fn execute_turn_accepts_valid_output_on_first_try() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: handle("runtime-1"),
                raw_output: valid_output("propose_progress", None, true),
                intent: intent(TransitionKind::ProposeProgress, None),
                usage: usage(),
                invalid_session: false,
            }],
        );

        let outcome = runtime
            .execute_turn(execute_turn_req(None))
            .expect("valid output should pass immediately");

        assert!(!outcome.resumed);
        assert_eq!(outcome.repair_count, 0);
        assert_eq!(outcome.session_reset_reason, None);
        assert_eq!(outcome.result.intent.kind, TransitionKind::ProposeProgress);
    }

    #[test]
    fn execute_turn_collects_changed_files_and_command_results() {
        let repo = temp_git_repo("coclai-observations");
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![ScriptedReply {
                handle: handle("runtime-observe"),
                raw_output: format!(
                    "{{\"work_id\":\"{WORK_ID}\",\"agent_id\":\"{AGENT_ID}\",\"lease_id\":\"{LEASE_ID}\",\"expected_rev\":9,\"kind\":\"propose_progress\",\"patch\":{{\"summary\":\"summary\",\"resolved_obligations\":[\"repair invalid json\"],\"declared_risks\":[]}},\"proof_hints\":[{{\"kind\":\"summary\",\"value\":\"summary\"}},{{\"kind\":\"file\",\"value\":\"tracked.txt\"}}]}}"
                ),
                intent: intent_with_hints(
                    TransitionKind::ProposeProgress,
                    None,
                    vec![crate::model::ProofHint {
                        kind: crate::model::ProofHintKind::File,
                        value: "tracked.txt".to_owned(),
                    }],
                ),
                usage: usage(),
                invalid_session: false,
            }],
        );

        let outcome = runtime
            .execute_turn(execute_turn_req_with(
                None,
                repo.display().to_string(),
                vec![GateCommandSpec {
                    applies_to_kind: TransitionKind::ProposeProgress,
                    argv: vec!["cargo".to_owned(), "--version".to_owned()],
                    timeout_sec: 5,
                    allow_exit_codes: vec![0],
                }],
            ))
            .expect("observation collection should succeed");

        assert_eq!(outcome.observations.changed_files.len(), 1);
        assert_eq!(outcome.observations.changed_files[0].path, "tracked.txt");
        assert_eq!(outcome.observations.command_results.len(), 1);
        assert_eq!(outcome.observations.command_results[0].exit_code, 0);
    }

    #[test]
    fn execute_turn_preserves_invalid_session_repair_after_observation_expansion() {
        let assets = RuntimeAssets::load_from_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("assets should load");
        let runtime = CoclaiRuntime::with_scripted_replies(
            assets,
            vec![
                ScriptedReply {
                    handle: handle("runtime-old"),
                    raw_output: valid_output("complete", Some("fixed"), true),
                    intent: intent(TransitionKind::Complete, Some("fixed".to_owned())),
                    usage: usage(),
                    invalid_session: true,
                },
                ScriptedReply {
                    handle: handle("runtime-new"),
                    raw_output: valid_output("complete", Some("fixed"), true),
                    intent: intent(TransitionKind::Complete, Some("fixed".to_owned())),
                    usage: usage(),
                    invalid_session: false,
                },
            ],
        );

        let outcome = runtime
            .execute_turn(execute_turn_req_with(
                Some(session(env!("CARGO_MANIFEST_DIR"))),
                env!("CARGO_MANIFEST_DIR").to_owned(),
                vec![GateCommandSpec {
                    applies_to_kind: TransitionKind::Complete,
                    argv: vec!["cargo".to_owned(), "--version".to_owned()],
                    timeout_sec: 5,
                    allow_exit_codes: vec![0],
                }],
            ))
            .expect("repair after invalid session should still collect observations");

        assert!(!outcome.resumed);
        assert_eq!(
            outcome.session_reset_reason,
            Some(SessionInvalidationReason::Runtime)
        );
        assert_eq!(outcome.observations.command_results.len(), 1);
    }

    fn scripted_invalid_reply(session_id: &str) -> ScriptedReply {
        ScriptedReply {
            handle: handle(session_id),
            raw_output: "{\"kind\":\"complete\"}".to_owned(),
            intent: intent(TransitionKind::Complete, Some("fixed".to_owned())),
            usage: usage(),
            invalid_session: false,
        }
    }

    fn temp_scripted_reply_file() -> std::path::PathBuf {
        let path = env::temp_dir().join(format!(
            "axiomnexus-scripted-runtime-{}.json",
            std::process::id()
        ));
        let reply = serde_json::json!([{
            "handle": { "runtime_session_id": "scripted-test" },
            "raw_output": valid_output("propose_progress", None, true),
            "intent": serde_json::from_str::<serde_json::Value>(&valid_output("propose_progress", None, true))
                .expect("valid output should parse"),
            "usage": {
                "input_tokens": 1,
                "output_tokens": 1,
                "run_seconds": 1,
                "estimated_cost_cents": 1
            },
            "invalid_session": false
        }]);
        fs::write(
            &path,
            serde_json::to_vec(&reply).expect("scripted reply json should encode"),
        )
        .expect("scripted reply file should write");
        path
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    fn usage() -> ConsumptionUsage {
        ConsumptionUsage {
            input_tokens: 120,
            output_tokens: 48,
            run_seconds: 3,
            estimated_cost_cents: Some(7),
        }
    }

    fn execute_turn_req(existing_session: Option<TaskSession>) -> ExecuteTurnReq {
        execute_turn_req_with(existing_session, "/repo".to_owned(), Vec::new())
    }

    fn execute_turn_req_with(
        existing_session: Option<TaskSession>,
        cwd: String,
        gate_plan: Vec<GateCommandSpec>,
    ) -> ExecuteTurnReq {
        ExecuteTurnReq {
            session_key: SessionKey {
                agent_id: AgentId::from(AGENT_ID),
                work_id: WorkId::from(WORK_ID),
            },
            cwd,
            existing_session,
            prompt_input: prompt_input(),
            gate_plan,
        }
    }

    fn prompt_input() -> PromptEnvelopeInput {
        PromptEnvelopeInput {
            snapshot: WorkSnapshot {
                work_id: WorkId::from(WORK_ID),
                company_id: CompanyId::from("company-1"),
                parent_id: None,
                kind: WorkKind::Task,
                title: "Implement runtime".to_owned(),
                body: "Connect coclai".to_owned(),
                status: WorkStatus::Doing,
                priority: Priority::High,
                assignee_agent_id: Some(AgentId::from(AGENT_ID)),
                active_lease_id: Some(LeaseId::from(LEASE_ID)),
                rev: 9,
                contract_set_id: ContractSetId::from("contract-1"),
                contract_rev: 1,
                created_at: SystemTime::UNIX_EPOCH,
                updated_at: SystemTime::UNIX_EPOCH,
            },
            unresolved_obligations: vec!["repair invalid json".to_owned()],
            contract_summary: "contract_rev=1".to_owned(),
            last_gate_summary: Some("note missing".to_owned()),
            last_decision_summary: Some("progress accepted".to_owned()),
        }
    }

    fn session(cwd: &str) -> TaskSession {
        TaskSession {
            session_id: SessionId::from("session-1"),
            company_id: CompanyId::from("company-1"),
            agent_id: AgentId::from(AGENT_ID),
            work_id: WorkId::from(WORK_ID),
            runtime: RuntimeKind::Coclai,
            runtime_session_id: "runtime-old".to_owned(),
            cwd: cwd.to_owned(),
            workspace_fingerprint: workspace_fingerprint(cwd),
            contract_rev: 1,
            last_record_id: None,
            last_decision_summary: Some("accepted".to_owned()),
            last_gate_summary: Some("failed".to_owned()),
            updated_at: SystemTime::UNIX_EPOCH,
        }
    }

    fn handle(runtime_session_id: &str) -> crate::port::runtime::RuntimeHandle {
        crate::port::runtime::RuntimeHandle {
            runtime_session_id: runtime_session_id.to_owned(),
        }
    }

    fn intent(kind: TransitionKind, note: Option<String>) -> TransitionIntent {
        intent_with_hints(kind, note, Vec::new())
    }

    fn intent_with_hints(
        kind: TransitionKind,
        note: Option<String>,
        mut extra_hints: Vec<crate::model::ProofHint>,
    ) -> TransitionIntent {
        extra_hints.insert(
            0,
            crate::model::ProofHint {
                kind: crate::model::ProofHintKind::Summary,
                value: "summary".to_owned(),
            },
        );
        TransitionIntent {
            work_id: WorkId::from(WORK_ID),
            agent_id: AgentId::from(AGENT_ID),
            lease_id: LeaseId::from(LEASE_ID),
            expected_rev: 9,
            kind,
            patch: WorkPatch {
                summary: "summary".to_owned(),
                resolved_obligations: vec!["repair invalid json".to_owned()],
                declared_risks: Vec::new(),
            },
            note,
            proof_hints: extra_hints,
        }
    }

    fn temp_git_repo(label: &str) -> std::path::PathBuf {
        let dir = env::temp_dir().join(format!(
            "axiomnexus-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("temp dir should exist");
        run_git(&dir, &["init"]);
        run_git(&dir, &["config", "user.email", "codex@example.com"]);
        run_git(&dir, &["config", "user.name", "Codex"]);
        fs::write(dir.join("tracked.txt"), "first\n").expect("tracked file should write");
        run_git(&dir, &["add", "tracked.txt"]);
        run_git(&dir, &["commit", "-m", "init"]);
        fs::write(dir.join("tracked.txt"), "second\n").expect("tracked file should rewrite");
        dir
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git command should run");
        assert!(status.success(), "git {:?} should succeed", args);
    }

    fn valid_output(kind: &str, note: Option<&str>, include_patch_arrays: bool) -> String {
        let patch = if include_patch_arrays {
            "\"patch\":{\"summary\":\"summary\",\"resolved_obligations\":[\"repair invalid json\"],\"declared_risks\":[]}"
        } else {
            "\"patch\":{\"summary\":\"summary\",\"resolved_obligations\":[],\"declared_risks\":[]}"
        };
        let note = note
            .map(|note| format!(",\"note\":\"{note}\""))
            .unwrap_or_default();
        format!(
            "{{\"work_id\":\"{WORK_ID}\",\"agent_id\":\"{AGENT_ID}\",\"lease_id\":\"{LEASE_ID}\",\"expected_rev\":9,\"kind\":\"{kind}\",{patch}{note},\"proof_hints\":[{{\"kind\":\"summary\",\"value\":\"summary\"}}]}}"
        )
    }
}
