# RuntimePort::execute_turn Spec

runtime boundary는 `execute_turn` 하나로 닫는다.

## 목표

runtime은 관측을 반환하고, kernel이 그 관측을 evidence로 해석한다.
runtime이 판정 규칙을 가지면 안 된다.

## trait

```rust
pub(crate) trait RuntimePort {
    fn execute_turn(
        &self,
        req: ExecuteTurnReq,
    ) -> Result<ExecuteTurnOutcome, RuntimeError>;
}
```

## 입력

`ExecuteTurnReq`는 아래 다섯 덩어리만 가진다.

1. `session_key`
2. `cwd`
3. `existing_session`
4. `prompt_input`
5. `gate_plan`

## `prompt_input`

runtime이 필요한 현재 문맥만 넘긴다.

- `snapshot`
- `unresolved_obligations`
- `contract_summary`
- `last_gate_summary`
- `last_decision_summary`

## `gate_plan`

runtime이 실행할 수 있는 command observation 계획이다.

```rust
pub(crate) struct GateCommandSpec {
    pub(crate) applies_to_kind: TransitionKind,
    pub(crate) argv: Vec<String>,
    pub(crate) timeout_sec: u64,
    pub(crate) allow_exit_codes: Vec<i32>,
}
```

중요:
- `argv` 배열을 그대로 쓴다
- `sh -c`를 쓰지 않는다
- 실행 결과는 판정이 아니라 관측이다

## 출력

`ExecuteTurnOutcome`은 아래를 포함해야 한다.

- runtime session handle
- parsed intent
- raw output
- usage
- invalid session 정보
- observations

## observations

`WorkspacePort` 없이 commit 직전까지 필요한 관측을 담는다.

- changed files
- command results
- artifact refs

## canonical schema

- transition intent schema: `samples/transition-intent.schema.json`
- execute-turn schema: `samples/execute-turn-output.schema.json`

문서, 코드, 테스트는 이 두 경로를 같은 기준으로 참조해야 한다.
