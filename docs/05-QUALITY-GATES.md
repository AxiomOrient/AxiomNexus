# 품질 게이트와 검증 전략

## 목적

이 시스템의 테스트는 기능 나열이 아니라 **불변식 증명 도구**여야 한다.

---

## 게이트 계층

### 1. 개발 기본 게이트

로컬 개발 중에는 아래만 먼저 본다.

```bash
scripts/verify-runtime.sh
```

이 스크립트는 아래 3개만 실행한다.

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

### 2. ship-now release gate

출시 가능 여부는 아래 한 스크립트로 본다.

```bash
scripts/verify-release.sh
```

이 스크립트는 아래를 포함한다.

```bash
scripts/verify-runtime.sh
cargo test transition_intent_schema_gate_is_live_contract
cargo test execute_turn_output_schema_gate_is_live_contract
scripts/smoke-runtime.sh
```

### 3. later hardening gate

preview release를 막지 않고 stable 이전에 별도로 닫는다.

- PostgreSQL adapter conformance
- dual-store conformance suite
- benchmark baseline
- extended observability audit

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

주의:
- scripted runtime smoke는 `AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME=1`이 있을 때만 허용한다.
- `AXIOMNEXUS_COCLAI_SCRIPT_PATH`만으로는 운영 runtime이 바뀌지 않아야 한다.

---

## later hardening 후보 도구

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

## ship-now 완료 조건

1. `scripts/verify-release.sh`가 통과한다.
2. runtime smoke가 queue → wake → diagnostic `run once <run_id>` → accepted complete → replay까지 돈다.
3. schema gate test가 `TransitionIntent`, `ExecuteTurnOutput` 둘 다 통과한다.
4. replay mismatch가 0이어야 한다.
5. 운영 기본값에서 scripted runtime 우회가 닫혀 있어야 한다.

## stable 추가 조건

1. adapter conformance suite가 Surreal과 PostgreSQL 모두에서 통과한다.
2. dual-store pass criteria가 `docs/spec/CONFORMANCE-SUITE.md`와 `plans/STABLE-BACKLOG.md`에 고정된다.
3. hot path benchmark baseline artifact가 저장된다.
4. observability audit가 끝나고 required span/event catalog가 문서화된다.
