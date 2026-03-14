# 목표 아키텍처

> 상태: 역사 참고용 문서.
> 현재 기준 문서는 `docs/01-FINAL-TARGET.md`, `docs/02-BLUEPRINT.md`, `docs/03-DOMAIN-AND-INVARIANTS.md`다.

이 문서는 현재 구현을 기준으로 다시 정리한 AxiomNexus v1 목표 설계입니다.

핵심 문장:

> AxiomNexus는 하나의 crate 안에서, 하나의 IDC kernel이 회사 소유 contract를 적용하고, 모든 상태 변경을 append-only transition record로 설명하는 control plane이다.

## 설계 원칙

1. 데이터 모델이 먼저 보인다.
2. 상태 전이 규칙은 `src/model`, `src/kernel`에만 둔다.
3. 외부 I/O는 `app -> port -> adapter`에서만 일어난다.
4. 모든 상태 변경은 `TransitionRecord`로 설명 가능해야 한다.
5. authoritative state 검증은 저장 시점에 다시 수행한다.
6. runtime은 coclai 하나만 전제로 단순화한다.
7. query와 transport는 contract를 비추되, business rule을 새로 만들지 않는다.

## 경계

| 경계 | 책임 |
| --- | --- |
| `model` | canonical types, ids, contracts, records |
| `kernel` | pure decision functions |
| `app` | use-case orchestration, evidence 수집 요청, kernel 호출 |
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

## 최소 핵심 데이터

- `WorkSnapshot`
- `WorkLease`
- `PendingWake`
- `TaskSession`
- `ContractSet`
- `EvidenceBundle`
- `TransitionIntent`
- `TransitionDecision`
- `TransitionRecord`

`TransitionRecord`는 reject/conflict 이유와 contract pin을 유실하지 않는 authoritative explanation source여야 합니다.

## 제어 흐름

### runtime submit

```text
load_context
  -> collect/observe evidence
  -> kernel::decide_transition
  -> store.commit_decision
```

중요한 점:

- evidence 수집은 side effect이므로 `app/port/adapter`에서 한다
- `kernel`은 `EvidenceBundle`을 평가만 한다

### wake

`wake`의 핵심은 queue fan-out이 아니라 work 단위 coalescing입니다.

현재 최종 계약:

- `PendingWake`는 work당 하나만 유지
- obligation은 deduped set으로 저장
- count는 wake event 수만 증가
- active lease/run이 없으면 runnable run 하나 생성

### scheduler / reaper

- timeout 경로도 `TransitionRecord`를 남겨야 한다
- system actor가 만든 전이도 replay 가능해야 한다
- runtime output은 같은 turn 안에서 commit까지 닫혀야 한다

## port 설계

- `StorePort`는 유지하되, call site는 역할별 trait로 좁힌다
- `RuntimePort`는 `execute_turn` 하나만 노출한다
- query read-model은 board/query 용도에만 사용한다

## 성공 기준

다음 네 가지가 동시에 성립하면 설계가 닫힙니다.

1. 모든 상태 변경이 `TransitionRecord`로 설명된다.
2. `commit_decision`이 `expected_rev`와 lease를 원자적으로 검증한다.
3. query/transport/app/adapter 경계가 책임 기준으로 일치한다.
4. 공식 빌드 게이트가 모두 통과한다.
