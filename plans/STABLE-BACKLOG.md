# STABLE-BACKLOG

## 목적

preview 이후 stable/final로 가기 위한 첫 실행 backlog를 고정한다.

---

## S-01 PostgreSQL adapter

- goal: `StorePort` semantic contract를 PostgreSQL adapter로 구현
- scope:
  - work / lease / pending_wake / run / task_session projection
  - transition_record / activity_event / consumption_event append path
  - export / import / replay parity
- done when:
  - claim / wake / commit / session / run persistence가 동작한다.
  - replay mismatch 0을 유지한다.
  - `docs/spec/CONFORMANCE-SUITE.md` 기준을 통과한다.

## S-02 Dual-store conformance

- goal: Surreal / PostgreSQL 둘 다 같은 의미론을 통과
- pass criteria:
  - same fixture set
  - same expected outcome
  - same typed error class
  - same replay result
  - same export/import semantic equality

## S-03 Benchmark baseline

- target paths:
  - `claim_lease`
  - `merge_wake`
  - `decide_transition`
  - `commit_decision`
  - `replay`
- artifact rule:
  - benchmark output은 repo-relative evidence 경로에 저장
  - stable 비교 기준은 직전 baseline과 같은 형식 유지

## S-04 Observability

- required span/event:
  - runtime turn start / finish
  - session resume / reset
  - decision accepted / rejected / conflict
  - commit transaction start / finish
  - replay mismatch
- done when:
  - event catalog가 문서화된다.
  - audit 스크립트가 최소 필수 span/event를 검증한다.
