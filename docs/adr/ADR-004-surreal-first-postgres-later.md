# ADR-004 — Surreal-first, PostgreSQL-later

## Status
Accepted

## Decision
개발 중 live store는 SurrealDB / SurrealKV를 유지하고, 의미론이 freeze 된 뒤 PostgreSQL adapter를 추가한다. [R1][S1][S2][PG1][PG2]

## Rationale
- 현재 저장소 baseline과 맞는다. [R1]
- embedded local dev 경험이 좋다. [S4]
- SurrealKV는 beta이므로 final production option은 추가로 필요할 수 있다. [S1]
- PostgreSQL은 constraints / transactions / concurrency semantics가 더 성숙하다. [PG1][PG2][PG4]

## Consequences
- portability 핵심은 `StorePort + Conformance Suite + Export/Replay`
- Surreal-specific fancy feature hot path 금지
