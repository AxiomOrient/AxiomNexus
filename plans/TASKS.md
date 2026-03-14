# TASKS

## 상태
- `todo`
- `doing`
- `blocked`
- `done`
- `defer`

---

## Done Ledger

| ID | Goal | Verification | Evidence | Status |
|---|---|---|---|---|
| NX-201 | `scheduler once`와 `run once` 역할 분리 문구 확정 | 문서가 operator path와 diagnostic path를 같은 말로 설명 | `README.md`, `docs/04-API-SURFACE.md`, `RELEASE-CHECKLIST.md` | done |
| NX-202 | README quick start 정리 | quick start가 canonical operator path와 diagnostic path를 구분 | `README.md` | done |
| NX-203 | API surface 문서 정리 | auto path 설명이 README와 일치 | `docs/04-API-SURFACE.md` | done |
| NX-204 | release checklist 용어 정리 | checklist와 smoke step 명칭이 일치 | `RELEASE-CHECKLIST.md`, `scripts/smoke-runtime.sh` | done |
| NX-205 | smoke step label 정리 | step log만 봐도 검증 경로가 보임 | `scripts/smoke-runtime.sh` | done |
| NX-206 | transition record direct assertion 추가 | accepted run 뒤 direct transition record gate 통과 | `scripts/smoke-runtime.sh` | done |
| NX-207 | task_session direct assertion 추가 | `GET /api/runs/{id}` / `GET /api/agents`로 session persistence 확인 | `scripts/smoke-runtime.sh` | done |
| NX-208 | consumption direct assertion 추가 | board / agent summary가 turn, token, cost를 직접 노출 | `scripts/smoke-runtime.sh` | done |
| NX-209 | replay assertion을 proxy와 direct assertion으로 분리 | replay가 integrity proxy gate로 분리 | `scripts/smoke-runtime.sh`, `RELEASE-CHECKLIST.md` | done |
| NX-210 | accepted/reject/repair smoke 문구 정리 | accepted, repair, rejected path가 step label에서 분리 | `scripts/smoke-runtime.sh` | done |
| NX-211 | local absolute path 제거 | 개인 로컬 절대 경로 0개 | `plans/README.md`, `plans/IMPLEMENTATION-PLAN.md`, `plans/TASKS.md`, `plans/STABLE-BACKLOG.md` | done |
| NX-212 | done-ledger / next-ledger 분리 | 완료 항목과 차기 항목이 혼재하지 않음 | `plans/TASKS.md` | done |
| NX-213 | preview blocker / stable backlog 분리 | preview 운영과 stable backlog가 분리됨 | `plans/IMPLEMENTATION-PLAN.md`, `plans/STABLE-BACKLOG.md` | done |
| NX-214 | evidence log를 repo-relative로 수정 | 모든 evidence 경로가 repo-relative | `plans/TASKS.md` | done |
| NX-215 | next-phase open decisions 명시 | open decision이 숨겨지지 않음 | `plans/IMPLEMENTATION-PLAN.md` | done |
| NX-216 | evidence pack script 추가 | 지정 경로에 release artifacts 생성 가능 | `scripts/export-release-evidence.sh` | done |
| NX-217 | release checklist placeholder 제거 | 실제 형식 경로 예시를 사용 | `RELEASE-CHECKLIST.md` | done |
| NX-218 | verify-release와 evidence pack 연결 | gate 실행 시 smoke log를 evidence dir로 저장 가능 | `scripts/verify-release.sh`, `scripts/export-release-evidence.sh` | done |
| NX-219 | release notes template 실제 절차와 연결 | release-notes가 evidence pack 일부가 됨 | `docs/RELEASE-NOTES-TEMPLATE.md`, `scripts/export-release-evidence.sh` | done |
| NX-220 | PostgreSQL adapter scope 문서화 | adapter 목표가 semantic contract 기준으로 적힘 | `plans/STABLE-BACKLOG.md` | done |
| NX-221 | dual-store conformance pass criteria 정의 | Surreal/PostgreSQL 공통 게이트 정의 | `docs/spec/CONFORMANCE-SUITE.md`, `plans/STABLE-BACKLOG.md` | done |
| NX-222 | benchmark baseline 계획 수립 | hot path benchmark 저장 방식이 정리됨 | `plans/STABLE-BACKLOG.md` | done |
| NX-223 | tracing/observability kickoff 문서화 | stable 이전 관측성 강화 기준 명시 | `docs/05-QUALITY-GATES.md`, `plans/STABLE-BACKLOG.md` | done |

---

## Next Ledger

| ID | Goal | Scope | Verification | Status |
|---|---|---|---|---|
| ST-301 | PostgreSQL adapter skeleton 구현 시작 | `src/adapter/postgres/*` | StorePort semantic contract의 최소 path가 compile | todo |
| ST-302 | dual-store conformance executable 추가 | adapter tests | Surreal/PostgreSQL이 같은 fixture를 통과 | todo |
| ST-303 | benchmark artifact 저장 명령 추가 | `scripts/`, benchmark harness | baseline이 repo-relative artifact로 남음 | todo |
| ST-304 | tracing/observability audit 스크립트 추가 | `scripts/`, docs | required span/event가 실제 로그로 검증됨 | todo |
