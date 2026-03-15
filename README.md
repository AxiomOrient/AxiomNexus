# AxiomNexus

AxiomNexus는 AI 소프트웨어 팀의 work/run 상태를 운영하고, 각 상태 전이를 계약과 증거로 판정하고 기록하는 control plane입니다. 회사 운영 OS를 넓게 만들려는 저장소가 아니고, 단순 코드 변경 승인기만으로도 축소하지 않습니다.

언어 포털: [KO](i18n/ko/README.md) | [ES](i18n/es/README.md) | [ZH](i18n/zh/README.md)

## 핵심 개념

에이전트는 상태를 직접 바꾸지 못합니다.
에이전트나 운영자는 `TransitionIntent`만 제출하고, 커널이 계약과 증거를 바탕으로 판정한 뒤, 저장소가 그 결과만 커밋합니다.

```
TransitionIntent  →  kernel decides  →  store commits
```

- 에이전트 자기보고보다 계약과 증거가 우선합니다.
- 모든 상태 전이 이유와 관찰된 gate evidence가 append-only `TransitionRecord`로 남습니다.
- 운영자 개입, wake, scheduler, runtime이 동일한 계약 규칙 위에서 동작합니다.
- 각 work는 `company_id + contract_set_id + contract_rev`에 고정되며, kernel이 이 세 값을 동시에 검사합니다. 다른 회사의 계약으로는 판정되지 않습니다.

## 제품 범위

- 이 저장소는 **runtime/control-plane 제품 코드만** 포함합니다.
- `triad`는 repo 내부 crate가 아니라 **외부 governance engine**으로 취급합니다.
- repo 안에 `axiomnexus-governance` workspace, `triad.toml`, repo-local claim pack, `.triad/*` bootstrap 자산은 두지 않습니다.
- 리뉴얼 기준은 호환 계층 유지보다 runtime 폐루프와 authoritative data contract를 먼저 닫는 것입니다.

## 하지 않는 것

- 회사 운영 OS로 범위를 넓히지 않습니다.
- PostgreSQL adapter를 이번 버전에 시작하지 않습니다.
- goals / budgets / org chart 같은 broad company surface를 이번 버전에 넣지 않습니다.
- 여러 runtime을 한 번에 일반화하지 않습니다.
- repo 안에 triad를 직접 넣어 함께 굴리지 않습니다.
- 범용 workflow builder나 plugin system을 이번 버전에 넣지 않습니다.

## 빠른 시작

**필요한 것**: Rust toolchain

기본 store URL: `surrealkv://.axiomnexus/state.db`

- 현재 live engine 기본값은 embedded SurrealKV입니다.
- 새 환경 변수는 `AXIOMNEXUS_STORE_URL`입니다.
- export 파일 경로는 `AXIOMNEXUS_EXPORT_PATH`이며 기본값은 `.axiomnexus/store_snapshot.json`입니다.
- `AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME`는 smoke/test 전용입니다. 배포 환경에서는 설정하지 않습니다.

```bash
cargo run -- migrate        # 스키마 적용
cargo run -- doctor         # DB 연결 및 상태 확인
cargo run -- contract check # 활성 계약 검증
cargo run -- serve          # HTTP 서버 시작 (기본: 127.0.0.1:3000)
cargo run -- scheduler once # 운영용 canonical queue consumer
cargo run -- run once run-2 # 특정 queued run을 직접 태우는 diagnostic path
scripts/verify-release.sh   # ship-now release gate + evidence pack export
```

빠른 시작에서 command 역할은 아래처럼 고정한다.

- `scheduler once`: 운영자가 queued run을 하나 소비시키는 canonical operator path
- `run once <run_id>`: 특정 queued run을 직접 재현하는 deterministic diagnostic path
- `scripts/verify-release.sh`: `.axiomnexus/releases/<version>/` 아래에 verify/smoke/replay/snapshot evidence를 남기는 release gate

## Preview workflow

내부 preview 운영은 아래 한 흐름으로 이해하면 된다.

1. `cargo run -- serve`로 API를 연다.
2. company, contract, agent, work를 만든다.
3. work를 `queue` 한다.
4. 운영자는 `cargo run -- scheduler once`로 queued run 하나를 소비시킨다.
5. `GET /api/activity`, `GET /api/runs/{id}`, `GET /api/work/{id}`로 결과를 확인한다.
6. `cargo run -- replay`로 현재 상태와 ledger가 맞는지 다시 검증한다.

문제가 있는 특정 queued run만 다시 볼 때는 `cargo run -- run once <run_id>`를 쓴다.

scripted smoke는 `scripts/smoke-runtime.sh` 내부에서만 아래 조합을 임시로 사용한다.

```bash
AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME=1
AXIOMNEXUS_COCLAI_SCRIPT_PATH=/tmp/scripted-replies.json
```

다른 포트:

```bash
AXIOMNEXUS_HTTP_ADDR=127.0.0.1:3001 cargo run -- serve
```

## 운영 표면

- HTTP/CLI로 회사, 계약, 에이전트, work, run, activity, SSE live stream을 다룹니다.
- 자동 실행 경로의 기준 use-case는 `run_turn_once`입니다.
- authoritative write path는 `Intent -> Decide -> Commit` 하나뿐입니다.
- replay는 `TransitionRecord`를 기준으로 snapshot 정합성을 검증하는 운영 도구여야 합니다.

## Preview evidence fields

- `artifact_refs`는 runtime이 파일, 로그, 외부 산출물 경로를 실제로 남길 때만 채워도 됩니다. preview smoke에서는 비어 있어도 실패가 아닙니다.
- `notes`는 operator에게 남길 짧은 관측 메모가 있을 때만 채웁니다. 값이 없으면 `null`이어도 됩니다.

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
- runtime intent schema: `samples/transition-intent.schema.json`
- runtime execute-turn schema: `samples/execute-turn-output.schema.json`

- `AGENTS.md`는 저장소 전반의 작업 규칙과 quality gate를 설명합니다.
- `.agents/AGENTS.md`와 skill 문서는 runtime agent prompt의 canonical source입니다.
- runtime prompt와 `contract check`, 테스트는 `.agents/*`와 schema 경로를 기준으로 동작합니다.

## CLI

| 명령 | 설명 |
|------|------|
| `migrate` | DB 스키마 적용 |
| `doctor` | DB 연결 및 환경 점검 |
| `contract check` | live active contract와 company binding 유효성 검사 |
| `serve` | HTTP 서버 시작 |
| `scheduler once` | canonical operator path로 가장 오래된 queued run 하나를 소비 |
| `run once <run_id>` | deterministic diagnostic path로 특정 queued run 하나를 직접 실행 |
| `replay` | live store에서 decision path 재진입 검증 |
| `export` | 현재 Surreal store snapshot을 JSON으로 저장 |
| `import` | snapshot JSON을 현재 Surreal store로 복원 |

## 품질 게이트

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
scripts/verify-runtime.sh
scripts/verify-release.sh
```

## 문서

- 실사용 흐름은 이 README의 `Preview workflow`를 먼저 본다.
- [문서 인덱스](docs/00-index.md) — canonical 읽기 순서
- [최종 도착지](docs/01-FINAL-TARGET.md) — 범위와 완료 조건
- [청사진](docs/02-BLUEPRINT.md) — 구조와 제어 흐름
- [도메인 모델과 불변식](docs/03-DOMAIN-AND-INVARIANTS.md) — 핵심 모델 기준
- [운영 표면](docs/04-API-SURFACE.md) — CLI / HTTP / SSE 기준
- [품질 게이트](docs/05-QUALITY-GATES.md) — 개발 / 릴리스 검증 순서
- [저장소 계약](docs/spec/STOREPORT-SEMANTIC-CONTRACT.md) — store 의미론 기준
- [runtime 계약](docs/spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md) — execute-turn 기준
- [adapter 검증 목록](docs/spec/CONFORMANCE-SUITE.md) — store 의미 검증 목록

## 현재 제한

- 단일 프로세스 SurrealKV runtime을 전제로 합니다.
- coclai 하나만 runtime adapter로 가정합니다.
