# Store Adapter Conformance Suite

이 문서는 store adapter가 같은 의미를 지키는지 확인하는 최소 검증 목록이다.

## 목적

같은 trait를 구현하는 것만으로는 충분하지 않다.
같은 전제, 같은 결과, 같은 실패 의미를 보여야 한다.

## 검증 묶음

### C1. lease semantics
- work당 active lease는 하나만 생성된다
- lease 충돌은 typed conflict로 드러난다

### C2. wake semantics
- wake는 work 단위로 coalescing 된다
- obligations는 deduped set을 유지한다
- count는 총 wake 횟수를 잃지 않는다

### C3. commit semantics
- rejected / conflict도 record를 append 한다
- accepted decision은 snapshot projection을 맞게 갱신한다
- stale rev를 잡는다
- lease mismatch를 잡는다

### C4. session semantics
- session은 `(agent_id, work_id)` 범위로 유지된다
- 최근 decision / gate 요약이 최신 값으로 갱신된다

### C5. replay semantics
- record stream으로 snapshot을 재구성할 수 있다
- mismatch를 탐지할 수 있다

### C6. export / import semantics
- export -> import 후 의미가 보존된다
- import 후 replay 결과가 같다

## 필수 대상

- Surreal adapter
- 이후 추가될 다른 persistent adapter

in-memory adapter는 보조 검증 대상으로 둘 수 있다.
