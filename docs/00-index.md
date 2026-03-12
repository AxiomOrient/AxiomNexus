# 문서 인덱스

이 인덱스는 `axiomnexus`의 active docs와 archive 경계를 고정합니다.

## Active Docs

- `README.md`: 저장소 진입점과 release-facing surface
- `01-system-design.md`: 제품/도메인/경계의 기준 문서
- `02-storage-review.md`: 저장소 선택 근거와 받아들인 제약
- `03-surrealdb-redesign.md`: persistence 구현 설계
- `05-target-architecture.md`: 현재 authoritative architecture
- `07-triad-governance-integration-plan.md`: `triad` governance integration surface

## First Scan Order

1. [../README.md](../README.md)
   - 무엇을 출시하는지
   - runtime surface와 governance surface가 어떻게 나뉘는지
   - 어디서 시작하면 되는지
2. [01-system-design.md](01-system-design.md)
   - AxiomNexus가 왜 필요한지
   - 어떤 경계와 규칙 위에서 동작하는지
   - 실제 사용 흐름이 무엇인지
3. [05-target-architecture.md](05-target-architecture.md)
   - 현재 어떤 구조를 authoritative로 볼지
   - 어떤 invariant가 release contract인지
4. [03-surrealdb-redesign.md](03-surrealdb-redesign.md)
   - persistence와 boot surface가 어떻게 고정됐는지
   - 어떤 저장소 topology를 전제로 하는지
5. [02-storage-review.md](02-storage-review.md)
   - 저장소 전환 판단이 무엇이었는지
   - 왜 SurrealKV를 기본 저장소로 고정했는지
   - 어떤 제약을 받아들이는지
6. [07-triad-governance-integration-plan.md](07-triad-governance-integration-plan.md)
   - triad를 어떤 방식으로 axiomnexus에 내장 사용할지
   - runtime code와 governance code, prompt assets, 문서 surface를 어디서 분리할지

## Archive

- [archive/README.md](archive/README.md): historical docs entrypoint
- [archive/04-implementation-review.md](archive/04-implementation-review.md): 날짜 고정 implementation review
- [archive/06-realignment-plan.md](archive/06-realignment-plan.md): 완료된 realignment plan
- [archive/08-triad-governance-tasks.md](archive/08-triad-governance-tasks.md): 완료된 governance task ledger

## Documentation Policy

- 루트 진입 설명은 `README.md` 하나에 둡니다.
- active docs는 현재 운영 surface와 architectural contract만 남깁니다.
- 날짜 고정 review, 완료된 plan, 완료된 task ledger는 `docs/archive/`로 이동합니다.
- repo-wide contributor rules는 루트 `AGENTS.md`에 둡니다.
- runtime agent prompt assets는 `.agents/AGENTS.md`와 `.agents/skills/**`에 둡니다.
- localized entry docs는 `docs/i18n/<lang>/README.md`에 둡니다.
