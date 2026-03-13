# REFERENCES

검증일: 2026-03-13 (Asia/Seoul)

## AxiomNexus 현재 저장소 기준

- [R1] AxiomNexus `README.md` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/README.md

- [R2] AxiomNexus `docs/05-target-architecture.md` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/docs/05-target-architecture.md

- [R3] AxiomNexus `docs/01-system-design.md` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/docs/01-system-design.md

- [R4] AxiomNexus `src/port/runtime.rs` (dev)  
  https://github.com/AxiomOrient/AxiomNexus/blob/dev/src/port/runtime.rs

- [R5] AxiomNexus `src/port/workspace.rs` (dev)  
  https://github.com/AxiomOrient/AxiomNexus/blob/dev/src/port/workspace.rs

- [R6] AxiomNexus `src/port/store.rs` (dev)  
  https://github.com/AxiomOrient/AxiomNexus/blob/dev/src/port/store.rs

- [R7] AxiomNexus `src/model/transition.rs` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/src/model/transition.rs

- [R8] AxiomNexus `src/model/session.rs` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/src/model/session.rs

- [R9] AxiomNexus `src/model/wake.rs` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/src/model/wake.rs

- [R10] AxiomNexus `samples/transition-intent.schema.json` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/samples/transition-intent.schema.json

- [R11] AxiomNexus `plans/IMPLEMENTATION-PLAN.md` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/plans/IMPLEMENTATION-PLAN.md

- [R12] AxiomNexus `plans/TASKS.md` (dev)  
  https://raw.githubusercontent.com/AxiomOrient/AxiomNexus/dev/plans/TASKS.md

## 외부 레퍼런스 프로젝트

- [P1] Paperclip GitHub README / repository page  
  https://github.com/paperclipai/paperclip

- [T1] triad `README.md`  
  https://raw.githubusercontent.com/AxiomOrient/triad/main/README.md

## 데이터 저장소 / 인프라 공식 문서

- [S1] SurrealKV 공식 문서 — beta 상태 명시  
  https://surrealdb.com/docs/surrealdb/installation/running/surrealkv

- [S2] SurrealQL Transactions 공식 문서  
  https://surrealdb.com/docs/surrealql/transactions

- [S3] SurrealDB `DEFINE INDEX ... UNIQUE` 공식 문서  
  https://surrealdb.com/docs/surrealql/statements/define/indexes

- [S4] SurrealDB Rust embedding 공식 문서  
  https://surrealdb.com/docs/surrealdb/embedding/rust

- [PG1] PostgreSQL Unique Constraints 공식 문서  
  https://www.postgresql.org/docs/current/ddl-constraints.html

- [PG2] PostgreSQL Transaction Isolation 공식 문서  
  https://www.postgresql.org/docs/current/transaction-iso.html

- [PG3] PostgreSQL Transactions tutorial 공식 문서  
  https://www.postgresql.org/docs/current/tutorial-transactions.html

- [PG4] PostgreSQL MVCC / Concurrency Control 공식 문서  
  https://www.postgresql.org/docs/current/mvcc.html

## Rust 라이브러리 공식 문서

- [AX1] axum 공식 문서  
  https://docs.rs/axum/latest/axum/

- [AX2] axum SSE `Event` / `KeepAlive` 문서  
  https://docs.rs/axum/latest/axum/response/sse/struct.Event.html  
  https://docs.rs/axum/latest/axum/response/sse/struct.KeepAlive.html

- [TP1] tokio-postgres 공식 문서  
  https://docs.rs/tokio-postgres

- [DP1] deadpool-postgres 공식 문서  
  https://docs.rs/deadpool-postgres

- [RF1] refinery 공식 문서  
  https://docs.rs/refinery/

- [TR1] tracing 공식 문서  
  https://docs.rs/tracing

- [PT1] proptest 공식 문서  
  https://docs.rs/proptest/latest/proptest/

- [CR1] criterion 공식 문서  
  https://docs.rs/criterion/latest/criterion/

- [IN1] insta 공식 문서  
  https://docs.rs/insta

- [SC1] schemars 공식 문서  
  https://docs.rs/schemars

- [JS1] jsonschema 공식 문서  
  https://docs.rs/jsonschema

## 레퍼런스 사용 규칙

1. `[R*]` 는 현재 AxiomNexus 저장소 상태를 설명할 때 사용한다.
2. `[P1]`, `[T1]` 는 외부 레퍼런스 프로젝트에서 가져올 primitive를 설명할 때 사용한다.
3. `[S*]`, `[PG*]` 는 저장소 전략과 adapter 전략의 근거로 사용한다.
4. `[AX*]`, `[TP1]`, `[DP1]`, `[RF1]`, `[TR1]`, `[PT1]`, `[CR1]`, `[IN1]`, `[SC1]`, `[JS1]` 는 구현 도구 선택의 근거로 사용한다.
