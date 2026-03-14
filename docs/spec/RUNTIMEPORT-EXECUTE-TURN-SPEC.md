# RuntimePort::execute_turn Spec

이 문서는 `WorkspacePort` 제거 이후의 최종 runtime boundary를 정의한다.

현재 저장소는 이미 `RuntimePort::execute_turn`을 사용한다. [R4]  
남은 과제는 `WorkspacePort`가 갖고 있는 관측 책임을 이 turn contract로 흡수하는 것이다. [R5]

---

## 1. 왜 `WorkspacePort`를 제거하는가

현재 `WorkspacePort`는 아래 세 가지만 한다. [R5]

- `current_dir`
- `observe_changed_files`
- `run_gate_command`

이 세 가지는 전부 **한 번의 runtime turn** 안에서 일어난 local action / local observation이다.

별도 port를 유지하면:
- evidence가 turn 밖으로 찢기고
- commit 직전 관측 단위가 두 개로 나뉘고
- runtime과 workspace가 같은 cwd/session를 공유한다는 사실이 타입으로 보이지 않는다

따라서 최종형에서는 제거한다.

---

## 2. 최종 trait

```rust
pub(crate) trait RuntimePort {
    fn execute_turn(
        &self,
        req: ExecuteTurnReq,
    ) -> Result<ExecuteTurnOutcome, RuntimeError>;
}
```

이 trait shape는 현재 저장소와 같고, output만 확장한다. [R4]

---

## 3. 최종 input

```rust
pub(crate) struct ExecuteTurnReq {
    pub(crate) session_key: SessionKey,
    pub(crate) cwd: String,
    pub(crate) existing_session: Option<TaskSession>,
    pub(crate) prompt_input: PromptEnvelopeInput,
    pub(crate) gate_plan: Vec<GateCommandSpec>,
}
```

### `PromptEnvelopeInput`
현재 저장소와 같은 정신 모델을 유지한다. [R4]

- `snapshot`
- `unresolved_obligations`
- `contract_summary`
- `last_gate_summary`
- `last_decision_summary`

### `gate_plan`
contract로부터 파생된 command observation plan이다.  
중요한 점은:

- `gate_plan`은 runtime intent kind별 command spec 묶음이다.
- runtime이 command를 실행할 수는 있다
- 하지만 command result가 “통과/실패”로 최종 판정되는 것은 kernel 단계다

```rust
pub(crate) struct GateCommandSpec {
    pub(crate) applies_to_kind: TransitionKind,
    pub(crate) argv: Vec<String>,
    pub(crate) timeout_sec: u64,
    pub(crate) allow_exit_codes: Vec<i32>,
}
```

---

## 4. 최종 output

현재 `ExecuteTurnOutcome`은 아래를 이미 포함한다. [R4]

- `handle`
- `result`
- `resumed`
- `repair_count`
- `session_reset_reason`
- `prompt_envelope`

최종형에서는 여기에 `observations`를 추가한다.

```rust
pub(crate) struct ExecuteTurnOutcome {
    pub(crate) handle: RuntimeHandle,
    pub(crate) result: RuntimeResult,
    pub(crate) resumed: bool,
    pub(crate) repair_count: u8,
    pub(crate) session_reset_reason: Option<SessionInvalidationReason>,
    pub(crate) prompt_envelope: String,
    pub(crate) observations: RuntimeObservations,
}

pub(crate) struct RuntimeResult {
    pub(crate) intent: TransitionIntent,
    pub(crate) raw_output: String,
    pub(crate) usage: ConsumptionUsage,
    pub(crate) invalid_session: bool,
}

pub(crate) struct RuntimeObservations {
    pub(crate) changed_files: Vec<FileChange>,
    pub(crate) command_results: Vec<CommandResult>,
    pub(crate) artifact_refs: Vec<EvidenceRef>,
    pub(crate) notes: Option<String>,
}
```

artifact carrier는 새 parallel type를 만들지 않고 existing evidence 계열 타입을 재사용한다.

---

## 5. 관측과 판정의 분리

이 분리가 핵심이다.

### runtime이 하는 일
- 모델과 대화
- session resume / repair
- parsed intent kind에 맞는 local commands 실행
- changed files 관측
- artifact 수집
- structured intent parsing
- raw output/usage 반환

### kernel/app이 하는 일
- observation → evidence 변환
- contract evaluation
- gate verdict 생성
- decision outcome 생성
- `TransitionRecord` 구성

이 구조는 `WorkspacePort` 제거와 `kernel decides` 원칙을 동시에 지킨다. [R1][R2][R4][R5]

---

## 6. session repair policy

현재 runtime 모델은 `invalid_session`과 `SessionInvalidationReason`을 이미 갖고 있다. [R4][R8]

최종 정책:
- invalid session이면 session clear 후 fresh start 1회 재시도
- `repair_count` 최대 1
- 최종 실패면 `RuntimeErrorKind::InvalidSession` 또는 `RuntimeErrorKind::InvalidOutput`

### scripted runtime guard

smoke/test용 scripted reply 주입은 운영 기본값에서 꺼져 있어야 한다.

- `AXIOMNEXUS_COCLAI_SCRIPT_PATH`만으로는 scripted runtime이 켜지지 않는다.
- `AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME=1`이 함께 있을 때만 scripted runtime을 허용한다.
- release gate는 이 guard가 유지되는지 테스트로 잠근다.

---

## 7. schema contract

이 문서의 machine-readable contract는 아래 파일이다.

- `samples/execute-turn-output.schema.json`
- `samples/transition-intent.schema.json`

schema drift는 `schemars` + `jsonschema`로 검증한다. [SC1][JS1]

---

## 8. 하지 않을 것

- runtime에게 최종 decision authority 주기
- runtime에게 DB write 책임 주기
- workspace 관측을 다시 별도 port로 분리하기
- triad verification을 runtime port 내부 primitive로 끌어들이기

---

## 9. 완료 조건

1. `WorkspacePort` 모듈이 제거된다.
2. changed files / command results / artifacts가 `execute_turn` 결과에서 보인다.
3. `run_turn_once` 가 별도 workspace port 없이 commit 직전 evidence assemble을 끝낸다.
4. invalid-session repair loop가 테스트로 잠긴다.
