# 최종 도착지

## 한 문장 정의

AxiomNexus는 계약과 증거로 work 상태 전이를 판정하고, 그 결과를 append-only `TransitionRecord`에 남기는 Rust 기반 IDC control plane이다.

## 제품의 중심

이 제품의 중심은 에이전트가 아니라 판정 경로다.

```text
TransitionIntent -> kernel::decide_transition -> store.commit_decision
```

따라서 최종 제품은 아래 질문에 답할 수 있어야 한다.

1. 지금 work 상태는 무엇인가
2. 왜 이 전이가 허용되었거나 거부되었는가
3. 같은 규칙을 다른 store adapter에서도 유지할 수 있는가
4. `TransitionRecord`만으로 상태를 다시 설명할 수 있는가

## 반드시 포함

- single-crate modular monolith
- `Intent -> Decide -> Commit` 단일 write path
- `TransitionRecord` append-only ledger
- `WorkLease`, `PendingWake`, `TaskSession` projection
- `RuntimePort::execute_turn`
- embedded SurrealKV 기본 개발 경로
- HTTP, CLI, SSE 운영 표면
- replay / export / import

## 제외

- business rule이 들어간 HTTP handler
- multi-runtime 일반화
- plugin system
- broad company OS surface
- repo-local triad workspace

## canonical assets

- repo rules: `AGENTS.md`
- runtime rules: `.agents/AGENTS.md`
- transition intent schema: `samples/transition-intent.schema.json`
- execute-turn schema: `samples/execute-turn-output.schema.json`

## 완료 조건

아래가 모두 참이어야 제품이 닫혔다고 본다.

1. 모든 상태 변경은 `TransitionRecord`로 설명된다.
2. 저장 시점에 `lease_id`와 `expected_rev`를 다시 검증한다.
3. runtime turn은 `WorkspacePort` 없이 필요한 관측을 모두 수집한다.
4. replay가 현재 snapshot과 맞아야 한다.
5. `StorePort` 의미 검증이 adapter마다 같은 결과를 낸다.

## 경계 결정

- triad는 내부 모듈이 아니라 외부 companion으로만 다룬다.
- PostgreSQL은 현재 기본 경로가 아니라 나중 adapter다.
- query 표면은 상태를 비추기만 하고 규칙을 새로 만들지 않는다.
