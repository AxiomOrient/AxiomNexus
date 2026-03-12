# SurrealDB 재설계

## 한 문장 요약

AxiomNexus의 저장소는 `StorePort`를 유지한 채, embedded SurrealKV 기반의 document-first adapter로 고정한다.

## 범위

이 설계가 다루는 범위:

- live persistence engine
- authoritative document set
- transactional commit hub
- snapshot backup/restore
- test seam과 boot surface

이 설계가 다루지 않는 범위:

- 상태 전이 규칙 변경
- HTTP contract 변경
- runtime adapter 변경
- graph/vector/live query 전면 도입

## 고정 조건

- `src/model`, `src/kernel`이 authoritative다.
- business rule은 storage adapter로 이동하지 않는다.
- `StorePort`는 sync contract로 유지한다.
- `transition_record`는 gate 결과와 observed evidence를 담는 append-only 설명 원본이다.
- 기본 topology는 단일 노드, 단일 프로세스다.
- 현재 pin은 `surrealdb = "=3.0.2"` + `features = ["kv-surrealkv"]`다.

## 저장소 topology

- 기본 store URL: `surrealkv://.axiomnexus/state.db`
- namespace: `axiomnexus`
- database: `primary`
- backup file: `.axiomnexus/store_snapshot.json`
- transition fallback: legacy `AXIOMS_*` env와 기존 `.axioms/` data dir도 읽을 수 있다

`SurrealStore` 내부에서만 Tokio runtime을 갖고, `StorePort` 바깥으로 async를 퍼뜨리지 않는다.

## 경계 설계

| 경계 | 책임 | 상태 |
| --- | --- | --- |
| `src/model` | canonical data contract | 유지 |
| `src/kernel` | 상태 전이 규칙 | 유지 |
| `src/app` | use-case orchestration | 유지 |
| `src/port/store.rs` | 저장소 계약 | 유지 |
| `src/adapter/memory` | 테스트/데모 seam | 유지 |
| `src/adapter/surreal` | live store + snapshot surface | 고정 |
| `src/boot` | storage URL 파싱, live boot | Surreal-only |

## 데이터 모델

authoritative/persisted set:

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
- `work_comment`
- `consumption_event`
- `activity_event`

`activity_event`는 projection이고, authoritative explanation source는 계속 `transition_record`다.
`transition_record`에는 gate 결과뿐 아니라 changed file observation, command result 같은 observed evidence도 함께 남긴다.

## 쓰기 경로

모든 command-side write는 아래를 유지한다.

`Load -> Kernel Decide -> Transactional Commit`

adapter는 판단하지 않는다.

- context를 읽는다.
- kernel이 판정한다.
- 판정 결과만 transaction으로 저장한다.

핵심 write hub는 `commit_decision`이다. 이 경로는 최소 아래를 한 transaction으로 다룬다.

1. `work`
2. `lease`
3. `pending_wake`
4. `task_session`
5. `transition_record`
6. `activity_event`
7. 필요 시 `run`

## 읽기 경로

현재 원칙은 두 가지다.

1. ID 기반 조회는 document read로 닫는다.
2. 관계가 많은 read surface는 projection을 읽되, 같은 table을 반복 scan하지 않도록 한 번에 묶는다.

현재 최적화된 대표 경로:

- `read_work`: pending wake, comment, activity를 batch/grouping해서 조합
- `read_agents`: recent run query와 consumption aggregation을 분리
- `read_activity`: 최근 20개만 query surface에서 제한

즉 Surreal 고유 기능은 “멋진 graph”보다 “저장 비용과 조회 비용이 보이는 query”에만 제한적으로 쓴다.

## snapshot backup/restore

공식 운영 표면은 Postgres cutover가 아니라 Surreal snapshot이다.

- format: `axiomnexus.surreal-snapshot.v1`
- checksum: `fnv64`
- export: 현재 Surreal store 전체 문서 집합 직렬화
- import: snapshot checksum 검증 후 전체 문서 집합 교체

이 표면의 목적은 migration이 아니라 백업과 복원이다.

## 왜 graph-first가 아닌가

현재 비용 구조가 graph traversal보다 아래에 가깝기 때문이다.

- work 하나를 기준으로 한 bounded update
- append-only record 추가
- read-model rollup
- ID 중심 조회

그래서 Surreal을 쓰더라도 첫 선택은 document-first가 맞다.

## 비목표

- multi-process writer
- distributed Surreal deployment
- SSE를 Surreal live query로 재구현
- vector search / graph edge 전면 도입
