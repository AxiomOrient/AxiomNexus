# 저장소 검토

기준 시점: 2026-03-11

## 질문

1. 이 프로젝트는 지금 어떤 저장소를 사용하는가
2. 왜 SurrealKV를 기본 저장소로 고정했는가
3. 어디까지가 단순 adapter 교체였고, 어디부터가 구조 변경이었는가

## 확인된 사실

### 1. 현재 런타임과 공식 backup/restore 표면은 Surreal-only다

- `Cargo.toml`은 runtime store로 `surrealdb = "=3.0.2"` 와 `features = ["kv-surrealkv"]`를 pin 한다.
- live 명령은 `AXIOMNEXUS_STORE_URL`만 사용한다.
- `serve`, `replay`, `migrate`, `doctor`는 embedded SurrealKV live surface다.
- `export`, `import`는 `AXIOMNEXUS_EXPORT_PATH` 기준의 Surreal snapshot backup/restore surface다.
- 과거 PostgreSQL bridge runtime/cutover surface는 현재 저장소에서 제거됐다.

즉 현재 저장소 현실은 다음 한 줄로 요약된다.

> AxiomNexus의 기본 runtime은 embedded SurrealKV document store이고, 공식 boot/backup surface도 Surreal snapshot 기준으로 닫혔다.

### 2. 전환이 단순했던 이유는 기존 구조가 이미 bridge 성격이었기 때문이다

전환 전에도 본질은 정규 relational 설계보다 아래에 가까웠다.

- ID 기반 조회
- work 중심의 bounded update
- append-only transition log
- read-model 조합

즉 핵심 문제는 “SQL을 얼마나 많이 쓰고 있나”가 아니라, “어떤 authoritative state를 어떤 contract로 커밋하나”였다.

### 3. 구조적으로 유지한 것과 바꾼 것은 명확하다

유지한 것:

- `StorePort`
- `Load -> Decide -> Commit`
- `src/model`, `src/kernel`의 계약
- append-only `transition_record`

바꾼 것:

- live persistence engine
- boot/config surface
- 테스트 seam
- backup/restore surface

## 판단

### 왜 SurrealKV를 고정했는가

이 저장소의 우선순위는 clone-and-run, 단일 프로세스 운영, contract-first write discipline이다.

그 기준에서 SurrealKV가 맞는 이유:

- embedded 실행이 가능하다.
- document-first 모델이 현재 write path와 잘 맞는다.
- multi-document transaction으로 `commit_decision` hub를 유지할 수 있다.
- `StorePort`를 바꾸지 않고 adapter 내부에서만 async bridge를 감출 수 있다.

### 왜 graph-first나 대규모 재설계는 하지 않았는가

현재 요구는 DB 기능 확장보다 상태 전이 규칙 보존이 우선이다.

- board
- scheduler
- runtime
- operator flow

이 경로들이 모두 같은 kernel 규칙을 써야 하므로, 저장소 교체는 document-first adapter 수준에서 끝내는 것이 가장 단순했다.

## 현재 결론

선택안은 확정됐다.

- 기본 저장소: embedded SurrealKV
- 기본 URL: `surrealkv://.axiomnexus/state.db`
- backup/restore: `.axiomnexus/store_snapshot.json`
- 테스트 seam: `MemoryStore`
- live adapter: `SurrealStore`
- 전환 호환성: legacy `AXIOMS_*` env와 기존 `.axioms/` data dir fallback 지원

즉 이 저장소는 더 이상 “Postgres에서 Surreal로 가는 중간 상태”가 아니다.  
현재 상태 자체가 **Surreal-only runtime**이다.

## 받아들인 제약

- 단일 노드, 단일 프로세스 운영
- `StorePort`는 sync 유지, adapter 내부에서만 Tokio runtime 사용
- Surreal local engine beta 표기는 수용하되 version pin으로 통제
- `/api/events`는 단일 프로세스 in-memory after-commit SSE fan-out까지 포함한다
