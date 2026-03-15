# StorePort Semantic Contract

이 문서는 store adapter가 반드시 같은 뜻으로 구현해야 하는 규칙만 남긴다.

## 원칙

1. `StorePort`는 CRUD 추상화가 아니다.
2. record가 권위고 snapshot은 projection이다.
3. accepted, rejected, conflict 모두 record를 남긴다.
4. 저장 시점 검증이 마지막 권위다.

## 핵심 연산

- `claim_lease`
- `load_context`
- `load_agent_facts`
- `load_session`
- `commit_decision`
- `merge_wake`
- `load_runtime_turn`
- `load_queued_runs`
- `read_*`
- `replay` 관련 조회

## 의미 규칙

### `claim_lease`
- active lease가 없을 때만 성공한다
- 충돌은 typed conflict로 드러난다
- lease projection과 work projection이 같이 맞아야 한다

### `commit_decision`
- record append가 먼저 설명 원본이 된다
- accepted일 때만 snapshot projection이 다음 상태로 간다
- `expected_rev`와 `lease_id`를 저장 시점에 다시 검증한다
- session, pending wake, activity 반영이 같은 commit 의미 안에 있어야 한다

### `merge_wake`
- work당 하나의 pending wake만 유지한다
- obligations는 deduped set을 유지한다
- count는 총 wake 횟수를 누적한다

### `load_runtime_turn`
- runtime이 필요한 snapshot, pending wake, contract pin을 한 번에 읽는다

### replay
- record stream으로 snapshot을 다시 만들 수 있어야 한다
- mismatch를 typed failure로 드러낼 수 있어야 한다

## role trait 원칙

role trait는 call-site를 좁히기 위한 용도다.
의미 규칙은 aggregate store surface 하나가 기준이다.

## adapter portability의 뜻

adapter portability는 저장 방식이 같은 것이 아니라 아래가 같은 것이다.

1. precondition
2. postcondition
3. failure class
4. replay 결과

이 검증 목록은 `CONFORMANCE-SUITE.md`가 맡는다.
