# Surreal Adapter Design

이 문서는 개발 중 authoritative store인 Surreal adapter의 최종 구현 기준을 정의한다. [R1]

---

## 1. 역할

Surreal adapter의 목표는 “Surreal답게 멋진 모델”이 아니다.  
목표는 **`StorePort` 의미론을 가장 빠르게, 가장 단순하게 닫는 기준 구현**이다. [S2][S3][S4]

---

## 2. 구현 원칙

### 2.1 embedded 우선
기본 URL은 README가 이미 선언한 `surrealkv://.axiomnexus/state.db` 를 따른다. [R1]

### 2.2 hot path는 transaction으로 묶는다
Surreal은 statement-level transaction과 explicit transaction을 지원한다. [S2]  
`commit_decision` 은 명시적 transaction으로 구현한다.

### 2.3 unique semantics는 index 또는 deterministic current record로 잠근다
Surreal은 unique index를 지원한다. [S3]  
다만 dev adapter를 단순하게 유지하기 위해 current-state projection은 아래처럼 설계하는 편이 좋다.

- `work:<work_id>`
- `lease:<work_id>`
- `pending_wake:<work_id>`
- `task_session:<agent_id>:<work_id>`
- `run:<run_id>`
- `transition_record:<record_id>`
- `activity_event:<event_id>`
- `consumption_event:<event_id>`

이렇게 하면 “current one row/record per key” semantics가 자연스럽게 보인다.

---

## 3. 권장 document sets

현재 README의 document set를 기준선으로 사용한다. [R1][R3]

필수:
- `store_meta`
- `company`
- `agent`
- `contract_revision`
- `work`
- `lease`
- `pending_wake`
- `run`
- `task_session`
- `transition_record`
- `activity_event`
- `consumption_event`

선택:
- `evidence_blob`

---

## 4. `claim_lease` 구현 원칙

### pre-read
- work snapshot read
- current lease record read
- agent facts read

### success path
- active lease가 없으면 `lease:<work_id>` current record write
- work snapshot의 `active_lease_id` projection sync
- activity emit optionally

### failure path
- existing active lease면 `StoreError::Conflict`

---

## 5. `merge_wake` 구현 원칙

- key는 `pending_wake:<work_id>`
- obligations는 set semantics로 merge [R9]
- count 증가
- latest_reason 갱신
- merged_at 갱신

### note
Surreal document model은 set merge 표현이 쉽지만, business meaning은 `BTreeSet<String>` 기준을 유지한다. [R9]

---

## 6. `commit_decision` 구현 원칙

### transaction steps
1. current work read
2. lease / expected_rev verify
3. append `transition_record`
4. apply snapshot projection if accepted / override accepted
5. apply lease effect
6. apply pending wake effect
7. upsert session if provided
8. append activity
9. commit

### 중요한 점
rejected / conflict도 record는 남긴다. [R1][R7]

---

## 7. export / import

현재 README는 export/import의 공식 표면을 Surreal snapshot JSON으로 둔다. [R1]

권장 구조:
- current-state projections
- append-only records
- schema version / export metadata
- checksum

### rule
export/import는 backup 도구이면서, 동시에 PostgreSQL later migration source이기도 하다.

---

## 8. replay

Surreal adapter에서 replay는 반드시 지원해야 한다.

입력:
- `transition_record` stream

출력:
- reconstructed snapshot

비교:
- live `work` projection

mismatch 발견 시 `doctor` / `replay` CLI에서 바로 보이게 해야 한다. [R1]

---

## 9. 하지 않을 것

- graph edge 중심 모델링
- live query를 authoritative mechanism으로 사용
- schema-less drift 허용
- Surreal-specific fancy feature를 hot path invariant에 넣기

---

## 10. 완료 조건

1. `claim_lease`, `merge_wake`, `commit_decision`, `load_context`, `load_runtime_turn` 구현
2. explicit transaction으로 atomic path 잠금
3. export/import 구현
4. replay 구현
5. conformance suite 통과
