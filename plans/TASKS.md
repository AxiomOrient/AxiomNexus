# TASKS

## 상태
- `todo`
- `doing`
- `blocked`
- `done`
- `defer`

## Phase A — Canonical Surface Cleanup

| ID | Goal | Scope | Verification | Depends On | Status |
|---|---|---|---|---|---|
| REL-001 | canonical reader path를 root README + docs index로 고정 | `README.md`, `docs/00-index.md` | 새 독자가 두 문서만으로 진입 가능 | none | done |
| REL-002 | `docs/README.md`를 축소 또는 제거 | `docs/README.md` | 더 이상 legacy docs를 active entrypoint처럼 소개하지 않음 | REL-001 | done |
| REL-003 | `docs/ARCHITECTURE.md`, `docs/API_CONTRACT.md`를 archive로 전환 | `docs/*` | AxiomSync / SQLite / `axiom://`가 active contract처럼 보이지 않음 | REL-002 | done |
| REL-004 | `crates/README.md`, `crates/axiomsync/README.md`를 legacy 표면으로 낮춤 | `crates/*` | root reader path에서 legacy surface 제거 | REL-003 | done |
| REL-005 | 존재하지 않는 release script 링크 제거 | docs + README | dead link 0개 | REL-002 | done |

## Phase B — Runtime Execute E2E Gate

| ID | Goal | Scope | Verification | Depends On | Status |
|---|---|---|---|---|---|
| REL-006 | smoke가 canonical auto path를 실제로 실행 | `scripts/smoke-runtime.sh` or new script | runtime execute가 accepted transition까지 간다 | REL-001 | done |
| REL-007 | accepted transition 후 snapshot rev 증가 검증 | smoke script + API read | `rev` increment assert | REL-006 | done |
| REL-008 | `TransitionRecord` append 검증 | smoke script + query/read path | latest activity / replay evidence 확인 | REL-006 | done |
| REL-009 | session 저장/갱신 검증 | smoke script + store/API | session continuity 확인 | REL-006 | done |
| REL-010 | consumption 기록 검증 | smoke script + store/API | usage/consumption event 확인 | REL-006 | done |
| REL-011 | invalid-session repair one-shot smoke 추가 | runtime integration or smoke | repair path works once then succeeds/fails deterministically | REL-006 | done |

## Phase C — Release Gates Split

| ID | Goal | Scope | Verification | Depends On | Status |
|---|---|---|---|---|---|
| REL-012 | ship-now gate 집합 정의 | docs + scripts | release owner가 필수 gate를 즉시 이해 | REL-006 | done |
| REL-013 | later hardening gate 집합 분리 | docs | postgres/benchmark가 preview release blocker가 아님 | REL-012 | done |
| REL-014 | schema drift test를 release gate에 명시 포함 | tests + scripts | `TransitionIntent`, `ExecuteTurnOutput` 둘 다 gate에 포함 | REL-012 | done |
| REL-015 | replay gate를 release 필수 항목으로 고정 | scripts + docs | release 시 replay mismatch 0 확인 | REL-012 | done |

## Phase D — Release Pack

| ID | Goal | Scope | Verification | Depends On | Status |
|---|---|---|---|---|---|
| REL-016 | release checklist 작성 | new release doc | checklist alone으로 release 수행 가능 | REL-012 | done |
| REL-017 | release note template 작성 | docs/template | 각 배포에 동일 형식 사용 | REL-016 | done |
| REL-018 | release evidence 저장 경로 고정 | docs + optional script | smoke/replay/export 결과 저장 위치 고정 | REL-016 | done |
| REL-019 | rollback 절차 문서화 | release doc | 이전 상태 복구 절차가 문서로 존재 | REL-016 | done |

## Phase E — Post-Release Hardening

| ID | Goal | Scope | Verification | Depends On | Status |
|---|---|---|---|---|---|
| REL-020 | PostgreSQL adapter backlog를 stable 작업으로 분리 | docs/plans | preview release blocker에서 분리 | REL-013 | done |
| REL-021 | conformance suite를 dual-store 기준으로 정의 | docs/tests backlog | later implementation 기준 확정 | REL-020 | done |
| REL-022 | benchmark baseline backlog 정의 | docs | hot path baseline 저장 방식 고정 | REL-020 | done |
| REL-023 | tracing/observability backlog 정의 | docs | stable 이전 관측성 강화 계획 분리 | REL-020 | done |

## Evidence Log

- `REL-001` `REL-005`: [README.md](/Users/axient/repository/AxiomNexus/README.md), [docs/00-index.md](/Users/axient/repository/AxiomNexus/docs/00-index.md), [docs/04-API-SURFACE.md](/Users/axient/repository/AxiomNexus/docs/04-API-SURFACE.md) 기준으로 canonical 진입점과 실제 CLI/API 표면을 맞췄고, 실제 없는 legacy 파일은 계획 문서에 정리 완료로 기록했다.
- `REL-006` `REL-011`: [scripts/smoke-runtime.sh](/Users/axient/repository/AxiomNexus/scripts/smoke-runtime.sh)에서 `queue → wake → run once <run_id> → accepted complete → invalid-session repair → replay`를 검증한다.
- `REL-012` `REL-015`: [scripts/verify-runtime.sh](/Users/axient/repository/AxiomNexus/scripts/verify-runtime.sh), [scripts/verify-release.sh](/Users/axient/repository/AxiomNexus/scripts/verify-release.sh), [docs/05-QUALITY-GATES.md](/Users/axient/repository/AxiomNexus/docs/05-QUALITY-GATES.md)로 ship-now / later hardening을 분리했고 schema/replay gate를 release 필수 항목으로 고정했다.
- `REL-016` `REL-019`: [RELEASE-CHECKLIST.md](/Users/axient/repository/AxiomNexus/RELEASE-CHECKLIST.md), [docs/RELEASE-NOTES-TEMPLATE.md](/Users/axient/repository/AxiomNexus/docs/RELEASE-NOTES-TEMPLATE.md)에 release evidence 경로와 rollback 절차를 고정했다.
- `REL-020` `REL-023`: [plans/ROADMAP.md](/Users/axient/repository/AxiomNexus/plans/ROADMAP.md), [plans/RELEASE-READINESS.md](/Users/axient/repository/AxiomNexus/plans/RELEASE-READINESS.md), [docs/05-QUALITY-GATES.md](/Users/axient/repository/AxiomNexus/docs/05-QUALITY-GATES.md)에 stable 이전 hardening backlog를 preview blocker와 분리해 기록했다.
