# PostgreSQL Adapter Design

이 문서는 **나중에 추가할** PostgreSQL adapter의 설계 문서다.

중요:
- 지금 당장 기본 store로 바꾸지 않는다.
- 먼저 Surreal 기준 구현과 semantic contract를 닫는다. [R1][S1]
- 이 문서는 later-phase adapter를 위한 설계 청사진이다.

---

## 1. 왜 PostgreSQL인가

PostgreSQL은 unique constraints, transaction isolation, MVCC semantics가 공식 문서 수준에서 명확하다. [PG1][PG2][PG3][PG4]  
AxiomNexus의 핵심 invariant는 이 강한 제약 모델과 잘 맞는다.

---

## 2. adapter 목적

PostgreSQL adapter의 목적은 다음이다.

1. production-grade persistence option 제공
2. stronger concurrency semantics 확보
3. analytics / ops ecosystem 친화성 확보
4. Surreal-first 개발과 semantic equivalence 유지

---

## 3. 권장 라이브러리

- `tokio-postgres` — async Postgres client [TP1]
- `deadpool-postgres` — pooled client + statement cache [DP1]
- `refinery` — SQL migration toolkit [RF1]

---

## 4. 권장 relational mapping

## current-state projections
```text
companies
agents
contract_revisions
works
leases
pending_wakes
runs
task_sessions
```

## append-only history
```text
transition_records
activity_events
consumption_events
evidence_blobs (optional)
```

---

## 5. key constraints

### works
- PK: `work_id`
- columns:
  - `company_id`
  - `status`
  - `rev`
  - `contract_set_id`
  - `contract_rev`
  - `active_lease_id`

### leases
단순화를 위해 current lease projection 테이블로 둔다.

- PK: `work_id`
- unique: `lease_id`
- semantics: row 존재 = active lease 존재

이 방식은 partial unique index보다 단순하며, Surreal current record model과도 잘 맞는다.

### pending_wakes
- PK / unique: `work_id`

### task_sessions
- unique: `(agent_id, work_id)`

### transition_records
- PK: `record_id`
- append-only application rule
- index:
  - `(work_id, happened_at)`
  - `(run_id)`
  - `(session_id)`

---

## 6. transaction strategy

### 기본 원칙
`commit_decision`은 단일 DB transaction 안에서 끝난다. [PG3]

### 권장 흐름
1. `BEGIN`
2. `SELECT ... FOR UPDATE` on `works` by `work_id`
3. current lease verify
4. `expected_rev` verify
5. insert `transition_records`
6. update `works` projection if accepted
7. update/delete `leases`
8. update/delete `pending_wakes`
9. upsert `task_sessions`
10. insert `activity_events`
11. `COMMIT`

### isolation
PostgreSQL 기본 isolation은 `READ COMMITTED` 다. [PG2]  
초기 adapter는 explicit row locking + constraints로 시작하고, conformance / concurrency tests 결과가 요구할 때만 higher isolation을 검토한다.

---

## 7. claim_lease strategy

두 가지 구현 방식이 가능하다.

### 권장 방식
- `INSERT INTO leases(work_id, lease_id, ...)`
- `work_id` PK/unique 충돌 시 conflict

이 방식은 “work당 active lease는 하나” semantics를 직접 보여 준다.

---

## 8. merge_wake strategy

- `INSERT ... ON CONFLICT (work_id) DO UPDATE`
- obligations set merge
- count increment
- latest_reason update
- merged_at update

주의:
- obligations는 canonical sorted set JSON으로 저장
- equality / replay 편의를 위해 deterministic serialization 유지

---

## 9. export/import and replay

PostgreSQL adapter는 Surreal과 같은 export/import/replay semantics를 통과해야 한다.

필수:
- same export JSON contract
- same replay result
- same failure classes

---

## 10. cutover strategy

1. Surreal export 생성
2. export validator 통과
3. PostgreSQL import
4. replay all works
5. mismatch 0 확인
6. read-only smoke
7. write-path smoke
8. cutover

---

## 11. 완료 조건

1. migration SQL 작성
2. adapter 구현
3. conformance suite 통과
4. replay / export / import 동등성 확보
5. documented cutover runbook 완성
