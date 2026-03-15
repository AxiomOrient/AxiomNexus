# 도메인 모델과 불변식

## 모델은 둘로 나눈다

### 현재 상태 projection
- `WorkSnapshot`
- `WorkLease`
- `PendingWake`
- `TaskSession`
- `Run`

### append-only 설명 원본
- `TransitionRecord`
- `ActivityEvent`
- `ConsumptionEvent`

현재 상태는 읽기 최적화용이고, 설명의 기준은 append-only record다.

## 핵심 타입

### `WorkSnapshot`
- 현재 work 상태
- 중요 필드: `status`, `rev`, `contract_set_id`, `contract_rev`, `active_lease_id`

### `WorkLease`
- 동시 실행 통제 projection
- work당 active lease는 최대 하나

### `PendingWake`
- work당 하나의 coalesced wake
- obligations는 set 의미를 유지
- count는 총 wake 횟수를 잃지 않는다

### `TaskSession`
- `(agent_id, work_id)` 단위 runtime 연속성 기록
- 최근 record, 최근 판정 요약, 최근 gate 요약을 들고 간다

### `TransitionIntent`
- 에이전트나 운영자가 제출하는 최소 전이 입력

### `TransitionRecord`
- 전이 이유, gate 결과, 관측된 증거를 남기는 설명 원본

## 항상 지켜야 할 규칙

1. 에이전트는 상태를 직접 바꾸지 못한다.
2. accepted, rejected, conflict 모두 record로 남긴다.
3. 각 work는 `company_id + contract_set_id + contract_rev`에 고정된다.
4. authoritative 판정은 kernel이 만들고 저장 시점에 다시 검증한다.
5. replay는 record stream만으로 snapshot을 재구성할 수 있어야 한다.
6. wake는 queue fan-out이 아니라 work 단위 coalescing을 유지한다.
7. 세션은 연속성을 돕지만 상태 권위는 아니다.

## 어디에 무엇을 두는가

- 타입 정의: `src/model`
- 상태 전이 계산: `src/kernel`
- context 조립과 commit 호출: `src/app`
