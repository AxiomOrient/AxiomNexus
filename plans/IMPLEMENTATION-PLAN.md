# IMPLEMENTATION-PLAN

## Scope
이 계획은 **새 아키텍처를 바꾸지 않는다**.
현재 구현을 출시 가능한 상태로 닫는 데만 집중한다.

---

## Workstream 1 — 문서 정합성 정리

### 변경 파일
- `README.md`
- `docs/00-index.md`
- `docs/04-API-SURFACE.md`
- `docs/05-QUALITY-GATES.md`
- `RELEASE-CHECKLIST.md`
- `docs/RELEASE-NOTES-TEMPLATE.md`

### 작업 내용
1. canonical docs path를 2개로 줄인다.
   - root `README.md`
   - `docs/00-index.md`
2. 실제 남아 있는 canonical 문서만 현재 구현과 맞춘다.
   - CLI `scheduler once` / `run once <run_id>`
   - `verify-runtime.sh` / `verify-release.sh`
   - release checklist / release note template
3. 이미 없는 legacy 파일은 “추가 정리 필요 없음”으로 ledger에 기록한다.
4. 존재하지 않는 스크립트(`quality_gates.sh`, `release_pack_strict_gate.sh`) 링크 제거

### 검증
- root README에서 따라간 링크가 모두 실제 파일을 가리킨다
- `docs/00-index.md`의 링크가 모두 살아 있다
- active docs 어디에도 `axiom://`, `context.db`, SQLite를 current product contract처럼 말하지 않는다

---

## Workstream 2 — runtime execute end-to-end smoke

### 변경 파일
- `scripts/smoke-runtime.sh`
- `scripts/verify-release.sh`
- `src/adapter/coclai/runtime.rs`
- `src/app/cmd/run_scheduler.rs`
- `src/boot/cli.rs`
- `src/boot/wire.rs`
- `src/lib.rs`

### 작업 내용
1. 현재 smoke가 직접 `/intents` rejected path만 보던 문제를 해결한다.
2. 아래 흐름을 실제로 실행한다.
   - company 생성
   - active contract 확인 / 활성화
   - agent 생성
   - work 생성
   - queue → wake
   - `run once <run_id>` 실행
   - accepted transition 확인
3. 이후 아래를 검증한다.
   - `TransitionRecord` append
   - `WorkSnapshot.rev` 증가
   - work status 기대값 반영
   - `run` 상태 completed
   - `task_session` 저장/갱신
   - `consumption_event` 또는 usage 기록
   - `replay` 통과
4. failure path도 한 번 더 본다.
   - invalid session repair
   - conflict/reject one-shot smoke

### 검증
스크립트 하나로 아래를 보장한다.

```bash
scripts/smoke-runtime.sh
```

---

## Workstream 3 — release gates split

### 변경 파일
- `README.md`
- `docs/05-QUALITY-GATES.md`
- `scripts/verify-runtime.sh`
- 필요 시 새 `scripts/verify-release.sh`

### 작업 내용
1. ship-now 게이트 정의
   - fmt
   - clippy
   - test
   - schema drift
   - replay
   - runtime e2e smoke
2. later hardening 게이트 정의
   - benchmark
   - PostgreSQL adapter conformance
   - extended observability audit
3. `verify-runtime.sh`는 ship-now gate만 보게 하거나,
   `verify-release.sh`와 역할을 분리한다.
4. README와 품질 문서에 두 계층을 명시한다.

### 검증
- 릴리스 담당자가 한 스크립트로 “출시 가능/불가”를 판단할 수 있다.
- later 항목이 preview release를 막지 않는다.

---

## Workstream 4 — release pack & rollback

### 변경 파일
- 새 `docs/RELEASE-CHECKLIST.md` 또는 루트 `RELEASE-CHECKLIST.md`
- 필요 시 `scripts/export-release-evidence.sh`
- release note template 파일

### 작업 내용
1. release checklist 작성
   - version
   - tag
   - smoke log
   - export snapshot
   - replay result
   - known limitations
2. rollback 절차 작성
   - export snapshot 복원
   - 이전 tag 재실행
   - migration 호환성 주의사항
3. release evidence 저장 경로 고정
   - `.axiomnexus/releases/<version>/...`

### 검증
사람이 바뀌어도 같은 절차로 릴리스를 재현할 수 있다.

---

## Workstream 5 — post-release hardening backlog

### 목적
preview 이후 stable을 위한 backlog를 미리 분리한다.

### 항목
- PostgreSQL adapter
- dual-store conformance suite
- benchmark baseline
- tracing event audit
- release automation

### 검증
preview release 직후 무엇을 해야 하는지가 다시 논쟁거리가 되지 않는다.
