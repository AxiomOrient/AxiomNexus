# StorePort Semantic Contract

이 문서는 AxiomNexus에서 **가장 중요한 문서**다.

이유는 단순하다.

- Surreal-first
- PostgreSQL-later

라는 전략이 성립하려면, 두 저장소가 같은 의미론을 구현해야 하기 때문이다. [S1][S2][PG1][PG2]

---

## 1. 설계 원칙

### 1.1 CRUD를 노출하지 않는다
`StorePort`는 table/document CRUD 추상화가 아니다.

좋은 예:
- `claim_lease`
- `load_context`
- `commit_decision`
- `merge_wake`

나쁜 예:
- `insert_lease`
- `update_work`
- `delete_pending_wake`

### 1.2 role trait는 call-site 좁히기 용도다
현재 저장소는 aggregate `StorePort`와 narrower role trait 구조를 이미 갖고 있다. [R6]  
이 방향은 유지한다.

### 1.3 record가 권위고 snapshot은 projection이다
`commit_decision`의 성공 기준은 “snapshot이 바뀌었다”가 아니라:

1. record가 append 되었고
2. effects가 적용되었으며
3. replay 가능성이 유지되는가

다.

---

## 2. 최종 trait 구조

```rust
pub(crate) trait StorePort:
    ControlPlaneStorePort
    + RuntimeStorePort
    + ReplayStorePort
    + QueryStorePort
{
}
```

### 2.1 ControlPlaneStorePort
핵심 mutation contract다.

```rust
pub(crate) trait ControlPlaneStorePort {
    fn claim_lease(&self, req: ClaimLeaseReq) -> Result<ClaimLeaseRes, StoreError>;
    fn load_context(&self, work_id: &WorkId) -> Result<WorkContext, StoreError>;
    fn load_agent_facts(&self, agent_id: &AgentId) -> Result<Option<AgentFacts>, StoreError>;
    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError>;
    fn commit_decision(&self, req: CommitDecisionReq) -> Result<CommitDecisionRes, StoreError>;
    fn merge_wake(&self, req: MergeWakeReq) -> Result<PendingWake, StoreError>;
}
```

### 2.2 RuntimeStorePort
runtime orchestration 보조 contract다. [R6]

```rust
pub(crate) trait RuntimeStorePort {
    fn claim_lease(&self, req: ClaimLeaseReq) -> Result<ClaimLeaseRes, StoreError>;
    fn load_runtime_turn(&self, run_id: &RunId) -> Result<RuntimeTurnContext, StoreError>;
    fn load_session(&self, key: &SessionKey) -> Result<Option<TaskSession>, StoreError>;
    fn save_session(&self, session: &TaskSession) -> Result<(), StoreError>;
    fn mark_run_running(&self, run_id: &RunId) -> Result<(), StoreError>;
    fn mark_run_completed(&self, run_id: &RunId) -> Result<(), StoreError>;
    fn mark_run_failed(&self, run_id: &RunId, reason: &str) -> Result<(), StoreError>;
    fn record_consumption(&self, req: RecordConsumptionReq) -> Result<(), StoreError>;
}
```

### 2.3 ReplayStorePort

```rust
pub(crate) trait ReplayStorePort {
    fn list_work_snapshots(&self) -> Result<Vec<WorkSnapshot>, StoreError>;
    fn load_transition_records(&self, work_id: &WorkId) -> Result<Vec<TransitionRecord>, StoreError>;
    fn export_state(&self, path: &str) -> Result<(), StoreError>;
    fn import_state(&self, path: &str) -> Result<(), StoreError>;
}
```

### 2.4 QueryStorePort
read model용이다.  
여기서는 성능과 transport 친화성이 중요하지만, business rule을 만들면 안 된다. [R2]

---

## 3. operation semantics

## 3.1 `claim_lease`

### preconditions
- work가 존재한다
- agent가 존재하고 실행 가능 상태다
- work가 lease 가능한 상태다

### success postconditions
- 해당 work의 current active lease projection이 정확히 하나 존재한다
- 반환된 `lease`는 authoritative current lease다

### failure classes
- `Conflict`: 다른 active lease가 이미 존재
- `NotFound`: work 또는 agent 부재
- `Unavailable`: backend error

### adapter invariants
- 같은 work에 대해 race가 나도 둘 이상의 active lease를 만들면 안 된다

---

## 3.2 `load_context`

반환해야 하는 최소 필드: [R6]

- `snapshot`
- `lease`
- `pending_wake`
- `contract`

이 호출은 decision 전 context assemble의 canonical source다.

---

## 3.3 `merge_wake`

### semantics
- work당 pending wake projection은 하나만 유지
- obligation은 deduped set으로 merge
- count는 event 수 증가
- latest_reason은 마지막 reason으로 갱신
- merged_at은 최신 시간으로 갱신

현재 저장소와 동일 의미다. [R2][R9]

---

## 3.4 `commit_decision`

이 연산이 가장 중요하다.

### input
- `TransitionDecision`
- `TransitionRecord`
- effects:
  - `lease_effect`
  - `pending_wake_effect`
  - optional `TaskSession`

현재 저장소의 `CommitDecisionReq` 구조와 정렬된다. [R6][R7]

### single logical transaction requirements
한 번의 logical transaction 안에서 아래가 성립해야 한다.

1. `expected_rev` 검증
2. active lease 검증
3. `TransitionRecord` append
4. accepted / override-accepted 면 snapshot projection 갱신
5. lease effect 적용
6. pending wake effect 적용
7. optional session upsert
8. activity event 반영

### allowed outcomes
- accepted
- rejected
- conflict
- override_accepted

### critical rule
rejected/conflict도 record는 남겨야 한다. [R1][R7]

### return value
- updated `snapshot` (if any)
- updated `lease` (if any)
- updated `pending_wake` (if any)
- updated `session` (if any)
- emitted `activity_event` (if any)

이는 현재 `CommitDecisionRes`와도 정렬된다. [R6]

---

## 4. current-state modeling rules

Store portability를 위해 아래 projection은 **current-state single row/record** 로 유지하는 것을 기본 원칙으로 둔다.

- `work`
- `lease`
- `pending_wake`
- `task_session`
- `run`

반대로 아래는 append-only다.

- `transition_record`
- `activity_event`
- `consumption_event`

이 원칙을 쓰면 Surreal에서도 PostgreSQL에서도 모델이 단순해진다.

---

## 5. export / import / replay

adapter portability는 schema abstraction보다 이 세 가지가 더 중요하다.

### export
현재 authoritative state와 append-only logs를 직렬화한다.

### import
같은 semantic contract를 만족하는 fresh store에 상태를 복원한다.

### replay
`TransitionRecord`로 `WorkSnapshot`을 재구성해 현재 projection과 비교한다.

---

## 6. error contract

```rust
pub(crate) enum StoreErrorKind {
    Conflict,
    NotFound,
    Unavailable,
}
```

현재 저장소와 정렬한다. [R6]

추가 error kind를 늘리기보다, message와 typed precondition violation mapping을 내부에서 정교하게 유지하는 편이 단순하다.

---

## 7. conformance complete condition

어떤 adapter든 아래가 통과해야 한다.

1. `claim_lease` race safety
2. `merge_wake` dedup safety
3. `commit_decision` atomicity
4. replay consistency
5. export/import fidelity

자세한 테스트는 `CONFORMANCE-SUITE.md`를 본다.
