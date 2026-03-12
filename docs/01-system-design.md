# 시스템 설계

## 한 문장 정의

AxiomNexus는 회사별 계약 아래에서만 work 상태 전이를 확정하는 IDC control plane입니다.

## 왜 존재하나

보통 에이전트 시스템은 에이전트가 "끝났다", "막혔다", "다음으로 넘겨라"라고 말하면 그 말을 어느 정도 신뢰합니다.  
AxiomNexus는 그 반대로 설계되었습니다.

- 에이전트는 상태를 직접 바꾸지 못합니다.
- 에이전트와 운영자는 `TransitionIntent`만 제출합니다.
- 실제 상태 변경은 contract, authoritative state, evidence를 통과한 경우에만 일어납니다.

즉 이 프레임워크의 목적은 "에이전트 실행기"가 아니라, "에이전트 작업을 통제하고 설명 가능하게 만드는 운영 제어면"입니다.

## 가장 단순한 정신 모델

모든 쓰기 경로는 아래 한 줄로 요약됩니다.

`Intent -> Decide -> Commit`

각 단계의 의미는 이렇습니다.

1. `Intent`
   - 에이전트 또는 board가 무엇을 하고 싶은지 선언합니다.
2. `Decide`
   - `kernel`이 현재 snapshot, contract, evidence를 보고 허용 여부를 판단합니다.
3. `Commit`
   - `store`가 `TransitionRecord`를 남기고 accepted 결과만 현재 상태에 반영합니다.

이 구조 덕분에 runtime, board, wake, scheduler, reaper가 서로 다른 편법으로 상태를 바꾸지 못합니다.

## 경계

| 경계 | 책임 |
| --- | --- |
| `src/model` | canonical data contract |
| `src/kernel` | 순수 상태 전이 규칙 |
| `src/app` | use-case orchestration |
| `src/port` | 외부 I/O 계약 |
| `src/adapter` | Surreal store, coclai, HTTP, SSE 구현 |
| `src/boot` | CLI/config/live boot wiring |

핵심 원칙은 간단합니다.

- business rule은 HTTP handler에 두지 않습니다.
- authoritative state는 store에서 읽습니다.
- 판정 규칙은 `model`, `kernel`에 둡니다.

## 핵심 데이터

### 현재 상태

- `WorkSnapshot`: 현재 work 상태
- `WorkLease`: 단일 active ownership
- `PendingWake`: follow-up obligation 집계
- `TaskSession`: runtime 연속성

### 설명과 감사

- `TransitionRecord`: gate 결과와 관찰된 evidence를 포함하는 append-only 설명 원본
- `ActivityEvent`: board와 work detail에서 읽는 운영 흔적
- `ConsumptionEvent`: turn-level usage 합산 원본

### 실행

- `Run`: queued/running execution 단위
- `ContractSet`: 회사별 상태 전이 규칙 집합

각 work는 `company_id`, `contract_set_id`, `contract_rev`에 고정됩니다.  
`kernel`은 판정 시 `contract.company_id`, `contract_set_id`, `revision`이 snapshot과 모두 일치하는 경우에만 전이를 허용합니다.  
즉 나중에 회사의 active contract가 바뀌어도 기존 work는 자기 revision 기준으로 계속 판정되며, 다른 회사 contract로는 판정되지 않습니다.

## 실제 사용 흐름

### 1. 운영자가 새 팀을 올리는 경우

1. 회사 생성
2. 그 회사의 contract draft 생성
3. contract activate
4. agent 생성
5. project/task 생성
6. board가 `queue` 또는 `wake`

실제 검증에서도 이 흐름을 그대로 태워서 새 회사가 자기 contract set으로 work를 만들고 queued run까지 생성하는 것을 확인했습니다.

### 2. 에이전트가 작업 완료를 주장하는 경우

1. 에이전트는 work detail에서 `rev`, `lease_id`, pending obligation을 읽습니다.
2. `TransitionIntent`를 제출합니다.
3. `kernel`이 evidence gate를 평가합니다.
   - file hint는 observed worktree change와 일치할 때만 changed-file evidence가 됩니다.
4. 통과하면 accepted, 부족하면 rejected 또는 conflict가 됩니다.

실제 검증에서는 changed file evidence 없이 `complete`를 보내면 거절됐습니다.  
즉 "에이전트가 끝났다고 말한 것"만으로는 done이 되지 않습니다.

### 3. 운영자가 에이전트를 멈추는 경우

1. agent를 `pause`
2. work에 `wake`
3. pending wake는 쌓이지만 queued run은 생기지 않음
4. agent를 `resume`
5. 다시 `wake`
6. 그때 queued run 생성

이 동작은 운영 통제 측면에서 중요합니다.  
실행을 멈추면서도 작업 요청은 잃지 않고, 재개 시점에 다시 runnable 상태로 만들 수 있기 때문입니다.

## 제품 표면

현재 노출된 표면은 다음과 같습니다.

- company create/read
- contract create/activate/read
- agent create/pause/resume/read
- work create/edit/queue/wake/reopen/cancel/override/read
- run read
- activity read
- board read
- after-commit SSE live route
- CLI `migrate`, `doctor`, `contract check`, `serve`, `replay`, `export`, `import`

`serve`는 현재 실제 TCP HTTP 서버를 bind합니다.

`/api/events`는 현재 단일 프로세스 in-memory after-commit 이벤트를 `text/event-stream`으로 broadcast합니다.
범위는 route/event contract + after-commit publish + live subscriber fan-out까지이며, durable backlog/replay는 포함하지 않습니다.

## 중요한 설계 선택

| 항목 | 선택 | 이유 |
| --- | --- | --- |
| 상태 변경 경로 | `Intent -> Decide -> Commit` 단일화 | drift를 줄이기 쉬움 |
| 런타임 | `coclai` 고정 | live path 단순화 |
| persistence | embedded SurrealKV document set | 현재 default live path |
| 감사 원본 | `TransitionRecord` append-only | replay와 설명 가능성 확보 |
| 보드/런타임 규칙 | 같은 contract 공유 | 운영 개입과 자동 실행을 같은 규칙에 묶음 |

## 현재 저장소 현실

현재 default live store는 embedded SurrealKV document set입니다.

- `SurrealStore`는 `store_meta`, `company`, `agent`, `contract_revision`, `work`, `lease`, `pending_wake`, `run`, `task_session`, `transition_record`, `work_comment`, `consumption_event`, `activity_event`를 authoritative/persisted set으로 사용합니다.
- `Load -> Decide -> Commit` 경로는 유지되고, kernel이 판정한 결과만 adapter가 transaction으로 저장합니다.
- `export`/`import`는 live store snapshot backup/restore surface로 고정했습니다.

즉 현재 persistence의 실체는 **embedded SurrealKV-backed document store**입니다.

2026-03-11 이후 저장소 surface는 embedded SurrealKV runtime + snapshot backup/restore로 수렴했습니다. 선택 근거는 [02-storage-review.md](02-storage-review.md), 기술 설계는 [03-surrealdb-redesign.md](03-surrealdb-redesign.md)에 분리합니다.

## 한계

- publication용 git remote는 아직 없습니다.

이 항목은 운영 메모이지, 현재 사용자 시나리오를 막는 핵심 기능 blocker는 아닙니다.
