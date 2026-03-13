# Store 전략 — Surreal-first, PostgreSQL-later

## 결론

개발 중 authoritative live store는 **SurrealDB / SurrealKV** 로 유지한다. [R1][S4]  
최종 production-grade adapter는 **PostgreSQL** 을 나중에 추가한다. [S1][PG1][PG2]

단, 이 전략은 아래 세 조건이 있을 때만 성립한다.

1. `StorePort` semantic contract가 먼저 고정된다.
2. adapter conformance suite가 존재한다.
3. export/import/replay가 공식 migration path로 존재한다.

---

## 1. 왜 지금 Surreal인가

현재 저장소의 active README는 기본 live engine을 embedded SurrealKV로 선언한다. [R1]  
SurrealDB는 Rust 안에서 embedded mode로 실행 가능하므로 local dev와 단일 프로세스 runtime에 유리하다. [S4]  
또한 transaction과 unique index를 제공한다. [S2][S3]

따라서 지금 당장 제품과 hot path를 닫는 기준 구현으로는 적절하다.

---

## 2. 왜 나중에 PostgreSQL인가

공식 문서 기준으로 SurrealKV storage engine 자체는 beta다. [S1]  
반면 PostgreSQL은 unique constraints와 transaction isolation semantics가 더 명확하고 성숙하다. [PG1][PG2][PG3][PG4]

따라서 최종 production-grade store adapter를 추가한다면 PostgreSQL이 가장 자연스럽다.

---

## 3. 이 전략이 성립하기 위한 금지 규칙

Surreal-first가 PostgreSQL-later로 연결되려면, dev 단계에서 아래를 금지해야 한다.

### 금지 1 — Surreal 고유 기능을 hot path authority로 사용
예:
- graph edge traversal을 authority로 사용
- time-travel query를 business invariant에 사용
- computed field를 authoritative state로 사용
- live query를 state machine 본체로 사용

### 금지 2 — document shape 유연성을 domain drift 허용에 사용
schema-less 편의는 dev 속도를 줄 수 있지만, final adapter portability를 해친다.

### 금지 3 — export/import 없이 store migration을 가정
migration은 “나중에 SQL로 다시 쓰면 된다”가 아니다.  
record, projection, replay semantics가 그대로 넘어가야 한다.

---

## 4. 권장 공통 모델링 원칙

### 4.1 domain id를 primary identity로 유지
Surreal record id 편의에 business 의미를 얹지 않는다.  
모든 canonical identity는 domain id 타입이 소유한다. [S3]

### 4.2 current-state projections는 단순한 table/document로 유지
- `work`
- `lease`
- `pending_wake`
- `task_session`
- `run`

### 4.3 append-only source는 분리
- `transition_record`
- `activity_event`
- `consumption_event`

### 4.4 원자성은 transaction으로 확보
Surreal은 manual transactions를 지원한다. [S2]  
PostgreSQL도 `BEGIN` / `COMMIT` transaction을 사용한다. [PG3]

---

## 5. StorePort portability의 실제 정의

DB를 바꿔 끼운다는 말의 실제 의미는 아래다.

1. 같은 `StorePort` method set
2. 같은 preconditions / postconditions
3. 같은 failure classes
4. 같은 replay result
5. 같은 export/import fidelity

즉 portability는 “SQL과 document DB 둘 다 가능”이 아니라 **semantic equivalence** 다.

---

## 6. 단계 전략

### 단계 A — Surreal semantic closure
- hot path 구현
- replay 구현
- export/import 구현
- conformance tests 작성

### 단계 B — PostgreSQL readiness
- relational mapping 설계
- transaction / lock strategy 설계
- same conformance suite 실행 준비

### 단계 C — PostgreSQL adapter
- adapter 구현
- 같은 tests 통과
- cutover / backfill 문서화

---

## 7. production cutover 기준

아래가 모두 성립하기 전에는 PostgreSQL cutover를 하지 않는다.

1. Surreal adapter conformance suite 100% 통과
2. replay mismatch 0
3. export/import roundtrip fidelity 100%
4. `TransitionIntent` / `ExecuteTurnOutput` schema freeze
5. `StorePort` contract freeze
6. PostgreSQL adapter가 동일 test suite 통과

---

## 8. 최종 판단

이 전략은 “처음부터 DB abstraction을 크게 잡자”가 아니다.  
오히려 반대다.

> **먼저 의미론을 잠그고, 그 다음 adapter를 추가한다.**

이게 단순하고, 나중에도 흔들리지 않는다.
