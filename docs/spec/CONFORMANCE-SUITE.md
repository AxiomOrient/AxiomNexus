# Store Adapter Conformance Suite

이 문서는 Surreal adapter와 PostgreSQL adapter가 **같은 의미론**을 구현하는지 검증하는 canonical suite를 정의한다.

---

## 1. 목표

같은 `StorePort` method set를 구현한다고 해서 adapter가 같은 시스템인 것은 아니다.  
진짜 중요한 것은 아래다.

- 같은 preconditions
- 같은 postconditions
- 같은 failure classes
- 같은 replay result

---

## 2. suite 구성

## C1. lease semantics
### C1-1 only_one_active_lease_per_work
- 동시에 두 claim을 던져도 active lease는 하나만 생성된다.

### C1-2 claim_conflict_is_typed
- 충돌은 `StoreErrorKind::Conflict` 로 드러난다.

## C2. wake semantics
### C2-1 wake_is_coalesced_per_work
- same work에 여러 wake가 와도 projection row/record는 하나다.

### C2-2 wake_obligations_are_deduped
- obligations는 set semantics를 유지한다. [R9]

### C2-3 wake_count_tracks_event_count
- count는 총 wake event 수를 잃지 않는다.

## C3. commit semantics
### C3-1 commit_decision_appends_record_even_when_rejected
- rejected / conflict 도 `TransitionRecord` 가 append 된다. [R1][R7]

### C3-2 accepted_decision_updates_snapshot_atomically
- record append와 snapshot 반영이 논리적으로 분리되지 않는다.

### C3-3 expected_rev_conflict_is_detected
- stale rev는 conflict 로 드러난다.

### C3-4 lease_mismatch_is_detected
- 다른 lease_id로 commit 불가.

## C4. session semantics
### C4-1 session_upsert_is_task_scoped
- `(agent_id, work_id)` unique semantics 유지

### C4-2 session_replace_preserves_latest_summary
- last gate/decision summary가 최신 값으로 유지

## C5. replay semantics
### C5-1 replay_reconstructs_snapshot
- record stream으로 current snapshot을 재구성 가능

### C5-2 replay_mismatch_is_detectable
- projection과 reconstructed snapshot mismatch를 탐지 가능

## C6. export/import semantics
### C6-1 export_import_roundtrip_is_lossless
- export → import → export 결과가 semantic equality를 가진다.

### C6-2 export_import_preserves_replay
- import 후 replay 결과 동일

---

## 3. 필수 실행 대상

- Surreal adapter
- PostgreSQL adapter
- in-memory fake adapter *(optional, kernel/app 테스트용)*

---

## 4. 테스트 환경

### Surreal
- embedded local database [S4]
- explicit transaction path [S2]

### PostgreSQL
- disposable local db
- migration applied by `refinery` [RF1]
- concurrency tests with multiple connections [TP1][DP1]

---

## 5. 통과 기준

어떤 adapter든:
- C1~C6 전체 통과
- replay mismatch 0
- typed error contract 동일
- same schema validator 통과

stable gate 기준:
- Surreal과 PostgreSQL이 같은 fixture set를 공유한다.
- 두 adapter 모두 같은 expected outcome과 replay result를 낸다.
- failure class와 export/import roundtrip 결과가 같아야 한다.

이 문서가 adapter portability의 최종 기준이다.
