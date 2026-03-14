# 도메인 모델과 불변식

## 모델 계층을 둘로 나눈다

AxiomNexus의 모델은 크게 둘이다.

### A. 현재 상태 projection
- `WorkSnapshot`
- `WorkLease`
- `PendingWake`
- `TaskSession`
- `Run`

### B. append-only explanation source
- `TransitionRecord`
- `ActivityEvent`
- `ConsumptionEvent`

이 분리가 중요하다.

- 현재 상태는 빠르게 읽기 위한 projection이다.
- authoritative explanation은 append-only record다. [R1][R2][R7]

---

## 핵심 타입

### `WorkSnapshot`
현재 work의 projection이다. [R3][R7]

핵심 필드:
- `work_id`
- `company_id`
- `status`
- `rev`
- `contract_set_id`
- `contract_rev`
- `active_lease_id`

주의:
- `WorkKind` / `Priority` / body/title 같은 필드는 hot path의 본질이 아니다.
- hot path의 본질은 `status`, `rev`, `contract pin`, `active lease`다.

### `WorkLease`
동시 실행 통제 projection이다.

권장 의미론:
- work당 active lease는 최대 하나
- history는 별도 lease history table이 아니라 `TransitionRecord`로 재구성 가능하면 충분
- dev store에서도 current lease projection은 work 기준 단일 row/record로 유지

### `PendingWake`
재실행 필요 신호의 coalesced projection이다. [R2][R9]

현재 저장소는 obligation을 deduped set으로 유지하고 count를 별도로 증가시키는 semantics를 이미 채택했다. [R9]

즉:
- queue fan-out 금지
- work당 pending wake는 하나
- obligations는 set
- count는 총 wake event 수
- latest_reason은 마지막 원인

### `TaskSession`
`(agent_id, work_id)` 단위 runtime continuity record다. [R8][R6]

현재 모델은 아래를 포함한다. [R8]

- `runtime_session_id`
- `cwd`
- `workspace_fingerprint`
- `contract_rev`
- `last_record_id`
- `last_decision_summary`
- `last_gate_summary`

이 구조는 유지하되, workspace fingerprint의 계산과 검증 책임은 runtime turn 쪽으로 이동시킨다.

### `TransitionIntent`
에이전트나 운영자가 제출하는 최소 상태 전이 입력이다. [R7][R10]

runtime-origin intent는 아래 kind만 허용한다. [R7][R10]

- `propose_progress`
- `complete`
- `block`

system/operator origin transition은 별도 `TransitionKind`를 사용할 수 있다. [R7]

### `TransitionDecision`
kernel이 내리는 판정이다. [R7]

핵심 필드:
- `outcome`
- `reasons`
- `next_snapshot`
- `lease_effect`
- `pending_wake_effect`
- `gate_results`
- `evidence`
- `summary`

### `TransitionRecord`
authoritative explanation source다. [R1][R2][R7]

핵심 역할:
- audit
- gate verdict 기록
- evidence container
- before/after explanation
- replay input

---

## 불변식

### INV-001 — One active lease per work
동일 work에 동시에 둘 이상의 active lease가 존재하면 안 된다.

### INV-002 — Wake is coalesced per work
wake는 queue가 아니라 work별 coalesced token이다. [R2][R9]

### INV-003 — Session continuity is task-scoped
같은 `agent + work` 에만 session이 resume된다. [R8]

reset trigger:
- agent mismatch
- work mismatch
- workspace mismatch
- runtime invalid session

### INV-004 — Intent is never authority
intent 자체는 state authority가 아니다.  
authority는 decision + commit 이후의 record다. [R1][R2]

### INV-005 — Record is append-only
`TransitionRecord`는 update/delete 하지 않는다. [R1][R7]

### INV-006 — Snapshot is replayable
현재 상태는 ledger replay로 재구성 가능해야 한다. [R1][R2]

### INV-007 — Contract pin must match
모든 work 전이는 `company_id + contract_set_id + contract_rev` pin에 맞아야 한다. [R1][R3]

### INV-008 — Runtime may observe, kernel decides
runtime은 command / file / artifact를 관측한다.  
gate verdict는 kernel이 만든다. [R2][R4][R5]

---

## 상태 전이 hot path

```text
claim lease
→ execute turn
→ collect evidence
→ decide transition
→ commit decision
→ append record
→ project snapshot/session/wake
```

이 흐름에서 핵심 데이터는 아래 다섯 개다.

- `WorkSnapshot`
- `WorkLease`
- `PendingWake`
- `TaskSession`
- `TransitionRecord`

Paperclip에서 가져올 primitive도 사실상 이 다섯 축과 겹친다.  
Paperclip는 atomic execution, persistent agent state, runtime skill injection, governance/audit를 강조하지만, 제품 표면은 훨씬 넓다. [P1]  
AxiomNexus는 primitive만 가져오고 제품 표면은 좁게 유지해야 한다.

---

## 현재 저장소와의 정렬

현재 저장소는 이미 아래를 선언한다. [R1][R2][R3]

- runtime/control-plane only
- append-only `TransitionRecord`
- coclai-only runtime
- SurrealKV default live engine
- triad external companion
- `Intent -> Decide -> Commit`

이번 문서 패키지는 이 선언을 **더 단순한 최종형으로 수렴**시킨다.
