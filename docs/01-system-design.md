# 시스템 설계

> 상태: 역사 참고용 문서.
> 현재 기준 문서는 `docs/01-FINAL-TARGET.md`, `docs/02-BLUEPRINT.md`, `docs/03-DOMAIN-AND-INVARIANTS.md`다.

## 한 문장 정의

AxiomNexus는 회사별 계약 아래에서만 work 상태 전이를 확정하는 IDC control plane입니다.

## 가장 단순한 정신 모델

모든 쓰기 경로는 아래 한 줄로 정리됩니다.

`Intent -> Decide -> Commit`

1. `Intent`
   - 에이전트 또는 운영자가 하고 싶은 전이를 선언합니다.
2. `Decide`
   - `kernel`이 현재 snapshot, contract, evidence를 보고 허용 여부를 판정합니다.
3. `Commit`
   - `store`가 `TransitionRecord`를 남기고 accepted 결과만 현재 상태에 반영합니다.

핵심은 에이전트가 상태를 직접 바꾸지 못한다는 점입니다.

## 경계

| 경계 | 책임 |
| --- | --- |
| `src/model` | canonical data contract |
| `src/kernel` | 순수 상태 전이 규칙 |
| `src/app` | use-case orchestration |
| `src/port` | 외부 I/O 계약 |
| `src/adapter` | Surreal store, coclai, HTTP, SSE 구현 |
| `src/boot` | CLI/config/live boot wiring |

원칙:

- business rule은 HTTP handler에 두지 않습니다.
- authoritative state는 store에서 읽습니다.
- 판정 규칙은 `model`, `kernel`에 둡니다.

## 핵심 데이터

### 현재 상태

- `WorkSnapshot`
- `WorkLease`
- `PendingWake`
- `TaskSession`
- `Run`
- `ContractSet`

### 설명과 감사

- `TransitionRecord`: gate 결과와 관찰된 evidence를 포함하는 append-only 설명 원본
- `ActivityEvent`: board와 work detail에서 읽는 운영 흔적
- `ConsumptionEvent`: turn-level usage 합산 원본

각 work는 `company_id`, `contract_set_id`, `contract_rev`에 고정됩니다.
`kernel`은 판정 시 이 값들이 snapshot과 모두 일치하는 경우에만 전이를 허용합니다.

## 실제 사용 흐름

### 운영자: 새 팀 온보딩

1. 회사 생성
2. contract draft 생성
3. contract activate
4. agent 생성
5. work 생성
6. queue 또는 wake

### 에이전트: 작업 완료 주장

1. work detail에서 `rev`, `lease_id`, unresolved obligation을 읽습니다.
2. `TransitionIntent`를 제출합니다.
3. `kernel`이 evidence gate를 평가합니다.
4. 통과하면 accepted, 부족하면 rejected 또는 conflict가 됩니다.

### 운영자: 실행 통제

1. agent를 `pause`
2. work를 `wake`
3. pending wake는 쌓이지만 queued run은 생기지 않음
4. agent를 `resume`
5. 다시 `wake`
6. queued run 생성

## 제품 표면

현재 노출된 표면:

- company create/read
- contract create/activate/read
- agent create/pause/resume/read
- work create/edit/queue/wake/reopen/cancel/override/read
- run read
- activity read
- board read
- after-commit SSE
- CLI `migrate`, `doctor`, `contract check`, `serve`, `replay`, `export`, `import`

## 저장소 현실

- 기본 live store는 embedded SurrealKV입니다.
- authoritative persistence는 `store_meta`, `company`, `agent`, `contract_revision`, `work`, `lease`, `pending_wake`, `run`, `task_session`, `transition_record`, `work_comment`, `consumption_event`, `activity_event` document set입니다.
- `export`/`import`는 live store snapshot backup/restore 표면입니다.

## 현재 제한

- 단일 프로세스 SurrealKV runtime을 전제로 합니다.
- coclai 하나만 runtime adapter로 가정합니다.
