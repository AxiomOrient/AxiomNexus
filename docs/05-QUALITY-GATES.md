# 품질 게이트와 검증 전략

## 목적

이 시스템의 테스트는 기능 나열이 아니라 **불변식 증명 도구**여야 한다.

---

## 빌드 게이트

현재 README가 선언한 기본 게이트는 유지한다. [R1]

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
scripts/smoke-runtime.sh
scripts/verify-runtime.sh
```

---

## 테스트 계층

### 1. kernel unit tests
pure function과 state rule을 검증한다.

예:
- `only_one_active_lease_per_work`
- `wake_is_coalesced_per_work`
- `runtime_intent_kind_is_runtime_only`
- `replay_reconstructs_snapshot`

### 2. adapter integration tests
각 store adapter가 semantic contract를 지키는지 검증한다.

- Surreal adapter
- PostgreSQL adapter

### 3. runtime integration tests
`execute_turn` 계약과 session repair를 검증한다.

- same task resume
- invalid session reset
- changed files capture
- gate command observation capture

### 4. replay tests
`TransitionRecord` 기반 상태 재구성을 검증한다.

### 5. end-to-end smoke tests
CLI + HTTP + runtime adapter가 함께 동작하는지 확인한다.

---

## 권장 도구

- `tracing` — structured event-based diagnostics [TR1]
- `proptest` — property testing [PT1]
- `criterion` — statistics-driven benchmarking [CR1]
- `insta` — snapshot tests [IN1]
- `schemars` — Rust type → JSON Schema [SC1]
- `jsonschema` — schema validation [JS1]

---

## 최소 필수 property tests

1. `claim_lease`는 어떤 interleaving에서도 work당 active lease를 둘 이상 만들지 않는다.
2. `merge_wake`는 obligation dedup semantics를 유지한다.
3. `commit_decision`은 accepted와 rejected를 동일 record contract로 처리한다.
4. replay는 record 순서가 같으면 snapshot이 항상 동일하다.

---

## 최소 필수 benchmark

`criterion` 기준으로 아래 path를 추적한다. [CR1]

- `claim_lease`
- `merge_wake`
- `decide_transition`
- `commit_decision`
- `replay`

---

## 관측성 기준

`tracing`으로 아래 span/event를 반드시 남긴다. [TR1]

- runtime turn start / finish
- session resume / reset
- lease acquired / conflict
- wake merged
- decision accepted / rejected / conflict
- commit transaction start / finish
- replay mismatch
- export / import

---

## 품질 완료 조건

1. unit + integration + replay + smoke가 모두 통과한다.
2. adapter conformance suite가 Surreal과 PostgreSQL 모두에서 통과한다.
3. replay mismatch가 0이어야 한다.
4. hot path benchmark baseline이 저장된다.
5. schema drift test가 `TransitionIntent`, `ExecuteTurnOutput` 둘 다 통과한다.
