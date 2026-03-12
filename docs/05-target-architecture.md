# 목표 아키텍처

이 문서는 현재 구현을 바탕으로 다시 정리한 **AxiomNexus v1 목표 설계문서**입니다.

핵심 문장:

> AxiomNexus는 하나의 crate 안에서, 하나의 IDC kernel이 회사 소유 contract를 적용하고, 모든 상태 변경을 append-only transition record로 설명하는 control plane이다.

## 1. 설계 원칙

1. 데이터 모델이 먼저 보인다.
2. 상태 전이 규칙은 `src/model`, `src/kernel`에만 둔다.
3. 외부 I/O는 `app -> port -> adapter`에서만 일어난다.
4. 모든 상태 변경은 `TransitionRecord`로 설명 가능해야 한다.
5. authoritative state 검증은 저장 시점에 다시 수행한다.
6. runtime은 coclai 하나만 전제로 단순화한다.
7. query와 transport는 contract를 비추되, business rule을 새로 만들지 않는다.

## 2. 경계

| 경계 | 책임 |
| --- | --- |
| `model` | canonical types, IDs, contracts, records |
| `kernel` | pure decision functions |
| `app` | use-case orchestration, evidence collection 요청, kernel 호출 |
| `port` | 외부 I/O trait |
| `adapter` | store/runtime/http/sse concrete implementation |
| `boot` | config, wiring, CLI entrypoint |

허용 방향:

- `boot -> app -> kernel -> model`
- `app -> port`
- `adapter -> port`
- `adapter -> model`
- `boot -> adapter`

금지 방향:

- `kernel -> adapter`
- `kernel -> app`
- `model -> adapter`
- `app::qry -> adapter::http`

## 3. 최소 핵심 데이터

### 필수 타입

- `WorkSnapshot`
- `WorkLease`
- `PendingWake`
- `TaskSession`
- `ContractSet`
- `EvidenceBundle`
- `TransitionIntent`
- `TransitionDecision`
- `TransitionRecord`

### 권장 `TransitionRecord`

현재 구현의 부족한 점을 메우기 위해, `TransitionRecord`는 아래 수준까지 authoritative explanation source가 되어야 합니다.

```rust
pub struct TransitionRecord {
    pub record_id: RecordId,
    pub company_id: CompanyId,
    pub work_id: WorkId,
    pub actor_kind: ActorKind,
    pub actor_id: ActorId,
    pub lease_id: Option<LeaseId>,
    pub expected_rev: Rev,
    pub contract_set_id: ContractSetId,
    pub contract_rev: ContractRev,
    pub before_status: WorkStatus,
    pub after_status: Option<WorkStatus>,
    pub outcome: DecisionOutcome,
    pub reasons: Vec<ReasonCode>,
    pub kind: TransitionKind,
    pub patch: WorkPatch,
    pub gate_results: Vec<GateResult>,
    pub evidence_inline: Option<EvidenceInline>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub happened_at: Timestamp,
}
```

핵심은 두 가지입니다.

- reject/conflict 이유가 유실되지 않아야 한다.
- activity projection이 `kind -> status` 하드코딩이 아니라 record 자체로부터 나와야 한다.

## 4. 제어 흐름

### 4.1 runtime submit

`submit_intent`는 계속 시스템의 중심 쓰기 경로입니다.

```text
load_context
  -> collect/observe evidence
  -> kernel::decide_transition
  -> store.commit_decision_cas
```

여기서 중요한 점:

- evidence 수집은 side effect이므로 `kernel`이 아니라 `app/port/adapter`에서 한다.
- `kernel`은 오직 `EvidenceBundle`을 평가만 한다.

이 배치는 현재 순수성 규칙과 더 잘 맞습니다.

### 4.2 claim

claim도 결국 하나의 IDC 상태 변경으로 보아야 합니다.

현재처럼:

- synthetic intent 생성
- kernel decide
- 별도 `claim_lease`
- 별도 `commit_decision`

로 쪼개기보다, 최종 형태는 아래에 가깝게 닫아야 합니다.

```text
load_context
  -> synthesize claim intent
  -> kernel::decide_transition
  -> store.commit_claim_decision_cas
```

즉 claim의 lease 취득과 snapshot 반영이 하나의 authoritative commit 안에서 함께 닫혀야 합니다.

### 4.3 wake

`wake`의 핵심은 queue fan-out이 아니라 **work 단위 coalescing**입니다.

규칙:

- `PendingWake`는 work당 하나만 유지
- obligation은 set merge
- count는 증가
- active lease/run이 없으면 runnable run 하나 생성

여기서 merge rule 자체는 `kernel`에 두고, run 생성은 adapter가 하더라도 정책 판단은 app/kernel에서 보여야 합니다.

### 4.4 scheduler / reaper

이 경로가 현재 가장 중요하게 정리되어야 합니다.

원칙:

- timeout으로 인한 lease release와 work 재가동도 IDC 밖에서 직접 snapshot을 바꾸면 안 된다.
- system actor가 만든 transition이어도 `TransitionRecord`는 남아야 한다.

즉 reaper는 "store 내부 보정 로직"이 아니라, **system-actor transition producer**여야 합니다.

실무적으로는 둘 중 하나를 택하면 됩니다.

1. `TransitionKind`에 system timeout/requeue kind를 추가한다.
2. existing kind로 표현 가능한 최소한의 system transition contract를 만든다.

어느 쪽이든 목표는 같습니다.

- direct snapshot mutation 제거
- timeout path도 replay 가능
- `transition_records`만으로 상태 이력을 설명 가능

## 5. port 설계

### 5.1 `StorePort`

aggregate `StorePort`는 유지하되, 실제 call site는 역할별 trait로 좁힙니다.

- context load
- command-side actor/company fact load
- claim/decision commit CAS
- wake merge persist
- session load/save
- runnable run load/mark
- read-model query

현재 구현은 `CommandStorePort`, `RuntimeStorePort`, `SchedulerStorePort`, `QueryStorePort`로 이 책임을 나눠서 사용합니다.

핵심은 "메서드 수를 줄이는 것"보다 "규칙을 숨기지 않는 것"입니다.

중요한 규칙:

- 전이 판정에 필요한 agent/company/lease 사실은 query projection이 아니라 command-side authoritative read에서 나와야 합니다.
- `read_agents()` 같은 read-model은 board/query 용도에만 사용합니다.

### 5.2 `RuntimePort`

현재 현실은 coclai 하나뿐입니다.  
따라서 포트도 그 현실을 반영해 단순해야 합니다.

현재 구현:

- public `RuntimePort`는 `execute_turn` 하나만 노출
- `start/resume/result`는 coclai adapter 내부 detail
- `RuntimeKind`는 session model에는 남지만 runtime port 계약에서는 제거

이렇게 하면 `submit_intent`에서 직접 process/git I/O를 돌리는 책임을 app 밖으로 밀어낼 수 있습니다.

### 5.3 제거된 빈 포트

`ClockPort`와 `BlobPort`는 v1 경로에 연결되지 않았기 때문에 제거합니다.

규칙:

- 실제 call path가 생길 때만 다시 추가
- 추상화보다 현재 데이터 흐름을 우선

선언만 있는 경계는 설계가 아니라 희망사항입니다.

## 6. query / transport 원칙

`app::qry` route-metadata stub는 제거했습니다.

현재 query path는 transport가 `QueryStorePort` read-model을 직접 비추는 구조입니다.
중간 상태의 fake query 계층보다 이쪽이 더 정직합니다.

HTTP adapter 원칙은 유지합니다.

- body parse
- path validate
- command app 호출
- query read-model serialize

다음 단순화 대상은 transport가 가진 read response shaping을 더 줄이는 것입니다.

## 7. Paperclip에서 유지할 것

Paperclip의 강점은 작업 실행기를 "상태 있는 운영 시스템"으로 다룬다는 점입니다.  
AxiomNexus는 그중 아래만 가져오면 충분합니다.

| 유지할 것 | AxiomNexus 해석 |
| --- | --- |
| persistent agent state | `TaskSession` |
| atomic execution | `WorkLease` + commit CAS |
| wake on a schedule | `PendingWake` + scheduler |
| auditable work loop | `TransitionRecord` |

가져오지 않을 것:

- 넓은 제품 표면
- generic multi-runtime abstraction
- runtime별 복잡한 capability matrix

## 8. 비목표

- workspace 분리
- 다중 runtime 일반화
- graph/vector 기능 확대
- wide project-management product surface
- HTTP handler에서 business rule 구현

## 9. 성공 기준

다음 네 가지가 동시에 성립하면 설계가 닫힙니다.

1. 모든 상태 변경이 `TransitionRecord`로 설명된다.
2. `commit_decision`이 `expected_rev`와 lease를 원자적으로 검증한다.
3. query/transport/app/adapter 경계가 책임 기준으로 일치한다.
4. 현재 공식 빌드 게이트가 모두 통과한다.
