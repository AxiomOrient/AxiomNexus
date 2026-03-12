# AxiomNexus

계약 우선 방식으로 에이전트 작업을 통제하는 Rust 기반 IDC control plane입니다.

언어 포털: [KO](docs/i18n/ko/README.md) | [ES](docs/i18n/es/README.md) | [ZH](docs/i18n/zh/README.md)

## 핵심 개념

에이전트는 상태를 직접 바꾸지 못합니다.
에이전트나 운영자는 `Intent`를 제출하고, 커널이 계약과 증거를 바탕으로 판정한 뒤, 저장소가 그 결과만 커밋합니다.

```
TransitionIntent  →  kernel decides  →  store commits
```

- 에이전트 자기보고보다 계약과 증거가 우선합니다.
- 모든 상태 전이 이유와 관찰된 gate evidence가 append-only `TransitionRecord`로 남습니다.
- 운영자 개입, wake, scheduler, runtime이 동일한 계약 규칙 위에서 동작합니다.
- 각 work는 `company_id + contract_set_id + contract_rev`에 고정되며, kernel이 이 세 값을 동시에 검사합니다. 다른 회사의 계약으로는 판정되지 않습니다.

## 빠른 시작

**필요한 것**: Rust toolchain

기본 store URL: `surrealkv://.axiomnexus/state.db`

- 현재 live engine 기본값은 embedded SurrealKV입니다.
- 새 환경 변수는 `AXIOMNEXUS_STORE_URL`입니다.
- export 파일 경로는 `AXIOMNEXUS_EXPORT_PATH`이며 기본값은 `.axiomnexus/store_snapshot.json`입니다.
- 전환 기간 동안 legacy `AXIOMS_*` env와 기존 `.axioms/` data dir도 fallback으로 읽습니다.

```bash
cargo run -- migrate        # 스키마 적용
cargo run -- doctor         # DB 연결 및 상태 확인
cargo run -- contract check # 활성 계약 검증
cargo run -- serve          # HTTP 서버 시작 (기본: 127.0.0.1:3000)
```

다른 포트:

```bash
AXIOMNEXUS_HTTP_ADDR=127.0.0.1:3001 cargo run -- serve
```

## 운영 표면

이 저장소에는 두 개의 명확한 운영 표면이 있습니다.

- runtime/control-plane: `axiomnexus` crate와 HTTP/CLI surface
- dev-governance: `crates/axiomnexus-governance`와 `triad.toml`, `spec/claims`, `.triad/*`

runtime은 제품 동작을 담당하고, governance는 저장소 변경 workflow를 관리합니다. 메인 `axiomnexus` package는 `triad-*`에 직접 의존하지 않습니다.

## 저장소 상태

- 현재 기본 구현은 **embedded SurrealKV를 사용합니다.**
- authoritative persistence는 `store_meta`, `company`, `agent`, `contract_revision`, `work`, `lease`, `pending_wake`, `run`, `task_session`, `transition_record`, `work_comment`, `consumption_event`, `activity_event` document set입니다.
- `export`/`import`의 공식 표면은 Surreal snapshot 파일입니다.
- runtime과 backup surface는 Surreal-only로 정리했습니다.

## Canonical Assets

현재 canonical asset과 rules surface는 아래처럼 역할을 나눠 고정합니다.

- repo-wide governance rules: `AGENTS.md`
- runtime agent prompt policy: `.agents/AGENTS.md`
- transition executor skill: `.agents/skills/transition-executor/SKILL.md`
- runtime output schema: `samples/transition-intent.schema.json`

- `AGENTS.md`는 저장소 전반의 작업 규칙과 quality gate를 설명합니다.
- `.agents/AGENTS.md`와 skill 문서는 runtime agent prompt의 canonical source입니다.
- runtime prompt와 `contract check`, 테스트는 `.agents/*`와 schema 경로를 기준으로 동작합니다.

## HTTP API

### 읽기

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/board` | 운영 보드 (work 상태 요약) |
| `GET` | `/api/companies` | 회사 목록 |
| `GET` | `/api/contracts/active` | 활성 계약 목록 |
| `GET` | `/api/agents` | 에이전트 목록 |
| `GET` | `/api/work` | work 전체 목록 |
| `GET` | `/api/work/{id}` | work 상세 (`rev`, `lease_id`, gate 결과 포함) |
| `GET` | `/api/runs/{id}` | run 상세 |
| `GET` | `/api/activity` | activity feed |
| `GET` | `/api/events` | after-commit live stream (`text/event-stream`) |

### 쓰기

| Method | Path | 설명 |
|--------|------|------|
| `POST` | `/api/companies` | 회사 생성 |
| `POST` | `/api/contracts` | 계약 draft 생성 |
| `POST` | `/api/contracts/{id}/activate` | 계약 activate |
| `POST` | `/api/agents` | 에이전트 생성 |
| `POST` | `/api/agents/{id}/pause` | 에이전트 pause |
| `POST` | `/api/agents/{id}/resume` | 에이전트 resume |
| `POST` | `/api/work` | work 생성 |
| `POST` | `/api/work/{id}/edit` | work 수정 |
| `POST` | `/api/work/{id}/queue` | board → todo (queue intent) |
| `POST` | `/api/work/{id}/wake` | follow-up obligation 등록 |
| `POST` | `/api/work/{id}/reopen` | done/cancelled → todo |
| `POST` | `/api/work/{id}/cancel` | work 취소 |
| `POST` | `/api/work/{id}/override` | 운영자 강제 완료 |
| `POST` | `/api/work/{id}/intents` | 에이전트 transition intent 제출 |

> `/api/events`는 단일 프로세스 in-memory after-commit 이벤트를 `text/event-stream`으로 broadcast합니다.

## 사용 흐름

### 운영자: 새 팀 온보딩

```
1. POST /api/companies          → company_id 발급
2. POST /api/contracts          → contract draft 생성
3. POST /api/contracts/{id}/activate
4. POST /api/agents             → agent_id 발급
5. POST /api/work               → work 생성
6. POST /api/work/{id}/queue    → todo로 전이
7. POST /api/work/{id}/wake     → run 생성, 에이전트에게 할당
```

### 에이전트: 작업 완료

```
1. GET  /api/work/{id}          → rev, lease_id, pending obligation 확인
2. POST /api/work/{id}/intents  → TransitionIntent 제출
   → kernel이 evidence gate 평가 (changed files, command result 등)
   → file hint는 세션 cwd에서 관찰된 변경과 일치할 때만 changed-file evidence로 인정
   → 통과: accepted + next snapshot 반영
   → 실패: rejected + gate 실패 이유 반환
```

### 운영자: 실행 통제

```
1. POST /api/agents/{id}/pause  → 이후 wake는 쌓이지만 run은 생성 안 됨
2. POST /api/work/{id}/wake     → pending wake만 추가
3. POST /api/agents/{id}/resume → 다음 wake부터 run 생성 재개
```

## CLI

| 명령 | 설명 |
|------|------|
| `migrate` | DB 스키마 적용 |
| `doctor` | DB 연결 및 환경 점검 |
| `contract check` | live active contract와 company binding 유효성 검사 |
| `serve` | HTTP 서버 시작 |
| `replay` | live store에서 decision path 재진입 검증 |
| `export` | 현재 Surreal store snapshot을 JSON으로 저장 |
| `import` | snapshot JSON을 현재 Surreal store로 복원 |

## Governance CLI

`triad` 기반 저장소 governance surface는 별도 crate로 분리돼 있습니다.

```bash
cargo run -p axiomnexus-governance -- init
cargo run -p axiomnexus-governance -- next
cargo run -p axiomnexus-governance -- status
cargo run -p axiomnexus-governance -- verify REQ-anx-001
```

governance runtime은 아래 artifact를 사용합니다.

- `triad.toml`
- `spec/claims/*.md`
- `.triad/evidence.ndjson`
- `.triad/patches/`
- `.triad/runs/`
- `.triad/schemas/`

현재 automated governance write scope는 `src/`, `docs/`, `samples/`, `.agents/`, `README.md`, `Cargo.toml`, `Cargo.lock`입니다. `README.md`를 포함하는 이유는 `REQ-anx-006`이 release entry surface를 claim 범위에 포함하기 때문입니다.

## 품질 게이트

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## 문서

- [문서 인덱스](docs/00-index.md) — active docs와 archive 경계
- [시스템 설계](docs/01-system-design.md) — 제품 경계와 핵심 데이터 모델
- [목표 아키텍처](docs/05-target-architecture.md) — 현재 authoritative architecture
- [Governance 통합 설계](docs/07-triad-governance-integration-plan.md) — `triad` integration surface
- [Archive 인덱스](docs/archive/README.md) — 완료된 계획, task ledger, 날짜 고정 review record

## 현재 제한

- publication용 git remote는 아직 없습니다.
