# TASKS.md

상태:
- `todo`
- `doing`
- `done`
- `defer`

## 우선순위 규칙
- P0: 이번 버전 완료를 막는 항목
- P1: preview 운영 전에 닫아야 하는 항목
- P2: 있으면 좋지만 preview blocker는 아닌 항목

---

## P0 — 제품 정체성 / operator path

### NX-401
우선순위: P0
제목: README 제품 정의 문구 잠금
목표:
- AxiomNexus를 “AI 소프트웨어 팀의 work/run control plane”으로 고정
변경 파일:
- `README.md`
완료 조건:
- README 첫 문단이 scope를 과하게 넓히거나 좁히지 않음
검증:
```bash
rg -n "control plane|회사 운영 OS|scheduler once|run once" README.md
```
증거:
- README 첫 문단에 `work/run control plane`, `회사 운영 OS 아님`, `단순 코드 변경 승인기만 아님`을 함께 고정
상태: done

### NX-402
우선순위: P0
제목: README quick start를 operator flow 기준으로 정리
목표:
- `serve -> queue -> scheduler once -> activity/replay` 흐름이 보이게 한다
변경 파일:
- `README.md`
완료 조건:
- 새 사용자가 README만 보고 preview 사용 순서를 이해 가능
검증:
```bash
rg -n "Preview workflow|scheduler once|run once" README.md
```
증거:
- README `Preview workflow`를 `serve -> company/contract/agent/work -> queue -> scheduler once -> activity/replay` 순서로 유지
상태: done

### NX-403
우선순위: P0
제목: `scheduler once` / `run once` 역할 문구 통일
목표:
- operator path와 diagnostic path를 혼동하지 않게 한다
변경 파일:
- `README.md`
- `scripts/smoke-runtime.sh`
- 필요 시 `src/boot/cli.rs`
완료 조건:
- 문서/CLI/스크립트가 같은 역할 정의를 사용
검증:
```bash
rg -n "scheduler once|run once" README.md src/boot scripts/smoke-runtime.sh
```
증거:
- README, `src/boot/cli.rs`, `scripts/smoke-runtime.sh`가 모두 `scheduler once = canonical operator path`, `run once <run_id> = deterministic diagnostic path`를 사용
상태: done

### NX-404
우선순위: P0
제목: scope drift 방지 문구 추가
목표:
- 이번 버전에서 하지 않을 것(회사 OS 확장, multi-runtime, PostgreSQL)을 짧게 못박는다
변경 파일:
- `README.md`
완료 조건:
- broad scope expansion 논의를 줄일 수 있는 짧은 “하지 않는 것” 섹션 존재
검증:
```bash
rg -n "하지 않는 것|PostgreSQL|multi-runtime|회사 운영 OS" README.md
```
증거:
- README `하지 않는 것`에 회사 운영 OS 확장 금지, PostgreSQL adapter 제외, multi-runtime 일반화 금지를 명시
상태: done

---

## P0 — release gate 직접 증거

### NX-405
우선순위: P0
제목: accepted transition direct assertion 강화
목표:
- smoke가 accepted path를 명시적으로 실패/성공 판정
변경 파일:
- `scripts/smoke-runtime.sh`
완료 조건:
- accepted transition 부재 시 smoke가 즉시 실패
검증:
```bash
scripts/smoke-runtime.sh
```
증거:
- `scripts/smoke-runtime.sh` step 8이 accepted complete transition detail 부재 시 즉시 실패하도록 확인
상태: done

### NX-406
우선순위: P0
제목: TransitionRecord append direct assertion 추가
목표:
- append-only ledger 기록을 직접 확인
변경 파일:
- `scripts/smoke-runtime.sh`
- 필요 시 read model/query surface
완료 조건:
- latest record 또는 record count 증가를 smoke가 확인
검증:
```bash
scripts/smoke-runtime.sh
```
증거:
- `scripts/smoke-runtime.sh` step 8이 `recent_transition_records` 증가와 complete record detail 존재를 직접 확인
상태: done

### NX-407
우선순위: P0
제목: WorkSnapshot revision 증가 direct assertion 유지/강화
목표:
- commit 이후 rev 증가를 before/after로 확인
변경 파일:
- `scripts/smoke-runtime.sh`
완료 조건:
- rev delta를 smoke output에서 확인 가능
검증:
```bash
scripts/smoke-runtime.sh
```
증거:
- `scripts/smoke-runtime.sh` step 8이 `before_rev` / `after_rev`를 비교해 rev 증가가 없으면 실패
상태: done

### NX-408
우선순위: P0
제목: task_session persistence direct assertion 추가
목표:
- session continuity가 실제 저장되는지 확인
변경 파일:
- `scripts/smoke-runtime.sh`
- 필요 시 session read model
완료 조건:
- session 존재/갱신을 조회 결과로 확인
검증:
```bash
scripts/smoke-runtime.sh
```
증거:
- `scripts/smoke-runtime.sh` step 8-9가 `current_session`과 `current_sessions` read model로 persisted task_session을 확인
상태: done

### NX-409
우선순위: P0
제목: consumption direct assertion 추가
목표:
- token/cost/turn summary를 직접 확인
변경 파일:
- `scripts/smoke-runtime.sh`
- 필요 시 consumption read model
완료 조건:
- agent 또는 global consumption summary에 값이 잡힘
검증:
```bash
scripts/smoke-runtime.sh
```
증거:
- `scripts/smoke-runtime.sh` step 9가 global/agent consumption summary의 turns, tokens, cost를 직접 확인
상태: done

### NX-410
우선순위: P0
제목: replay integrity assertion 강화
목표:
- replay success를 integrity gate로 명확히 유지
변경 파일:
- `scripts/verify-release.sh`
- `scripts/smoke-runtime.sh`
완료 조건:
- replay 실패 시 verify-release 전체 실패
검증:
```bash
scripts/verify-release.sh
```
증거:
- `scripts/smoke-runtime.sh` step 12 replay gate가 살아 있고, `scripts/verify-release.sh` 통과가 replay 성공에 의존함
상태: done

---

## P1 — HTTP purity / Surreal contract

### NX-411
우선순위: P1
제목: HTTP handler business rule grep 점검
목표:
- handler가 app orchestration만 하도록 확인
변경 파일:
- 없음 또는 최소 보정
확인 파일:
- `src/adapter/http/**`
완료 조건:
- 아래 grep이 비어 있거나 test-only 결과만 나옴
검증:
```bash
rg -n "kernel::|ReasonCode|decide_transition|transition_record" src/adapter/http
```
증거:
- grep 결과는 `src/adapter/http/transport.rs` test-only 문자열만 남고 production handler business rule 호출은 없음
상태: done

### NX-412
우선순위: P1
제목: Surreal commit_decision save-time validation 점검
목표:
- record-first / stale rev / lease mismatch 계약 확인
확인 파일:
- `src/adapter/surreal/**`
완료 조건:
- commit semantics가 문서 계약과 어긋나지 않음
검증:
```bash
rg -n "fn commit_decision|expected_rev|lease_id|append.*record|transition_record" src/adapter/surreal
```
증거:
- `src/adapter/surreal/store.rs`와 `store/commit.rs`에 record-first transaction, `expected_rev`, `lease_id` 검증 경로가 존재
- 관련 회귀 테스트 `commit_decision_rejects_stale_expected_rev`, `commit_decision_rejects_stale_live_lease`, `surreal_commit_transaction_appends_record_before_projection_updates`가 통과
상태: done

### NX-413
우선순위: P1
제목: 필요한 경우 adapter test 메시지 보강
목표:
- 실패 시 어떤 계약이 깨졌는지 즉시 알 수 있게 한다
변경 파일:
- 관련 테스트 파일
완료 조건:
- 테스트 실패 메시지가 추상적이지 않음
검증:
```bash
cargo test
```
증거:
- 기존 adapter 테스트 이름과 실패 메시지가 계약 단위를 직접 가리켜 추가 수정 없이 `cargo test` 통과
상태: done

---

## P1 — preview release evidence

### NX-414
우선순위: P1
제목: preview evidence pack 경로 고정
목표:
- `.axiomnexus/releases/<version>/`를 공식 evidence 경로로 사용
변경 파일:
- `scripts/verify-release.sh`
- 필요 시 새 `scripts/export-release-evidence.sh`
완료 조건:
- verify/replay/smoke 로그와 snapshot이 저장됨
검증:
```bash
scripts/verify-release.sh
ls -R .axiomnexus/releases
```
증거:
- `scripts/verify-release.sh`가 기본 경로 `.axiomnexus/releases/<version>/`를 만들고 `verify-release.log`, `smoke-runtime.log`, `replay.log`, `store_snapshot.json`을 저장
상태: done

### NX-415
우선순위: P1
제목: preview release evidence 최소 구성 정의
목표:
- 어떤 파일이 evidence pack에 들어가야 하는지 고정
- 최소 구성:
  - `verify-release.log`
  - `smoke-runtime.log`
  - `replay.log`
  - `store_snapshot.json`
  - 필요 시 `release-notes.md`
변경 파일:
- `plans/07_IMPLEMENTATION_PLAN.md`
- `plans/TASKS.md`
완료 조건:
- 팀 내에서 evidence 누락으로 재논쟁하지 않음
검증:
- 문서 검토
증거:
- `plans/07_IMPLEMENTATION_PLAN.md`와 `plans/TASKS.md`에 evidence pack 최소 파일 집합을 동일하게 고정
상태: done

---

## P2 — 유지/정리

### NX-416
우선순위: P2
제목: `artifact_refs` / `notes` preview 기대치 문서화
목표:
- 비어 있어도 되는지, 언제 채워야 하는지 기대치를 명확히
변경 파일:
- `README.md` 또는 runtime 관련 문서
완료 조건:
- 운영자가 필드 의미를 오해하지 않음
검증:
```bash
rg -n "artifact_refs|notes" README.md docs src/port/runtime.rs
```
증거:
- README `Preview evidence fields`와 runtime spec에 `artifact_refs` / `notes`의 preview 기대치를 명시
상태: done

### NX-417
우선순위: P2
제목: demo seed가 release path에 섞이지 않는지 확인
목표:
- scripted/demo helper와 실제 preview 경로 분리
확인 파일:
- `src/adapter/surreal/**`
- `scripts/smoke-runtime.sh`
완료 조건:
- smoke/test 전용 helper가 운영 경로를 오염시키지 않음
검증:
```bash
rg -n "seed_demo_state|ALLOW_SCRIPTED_RUNTIME|COCLAI_SCRIPT_PATH" src scripts
```
증거:
- scripted env는 `scripts/smoke-runtime.sh`와 coclai runtime test gate에만 등장하고, README도 smoke/test 전용임을 명시
- `seed_demo_state`는 demo bootstrap helper에만 남아 release/operator path 기본값으로 쓰이지 않음
상태: done

---

## 다음 버전으로 이월

### NX-418
우선순위: defer
제목: PostgreSQL adapter
상태: defer

### NX-419
우선순위: defer
제목: dual-store conformance
상태: defer

### NX-420
우선순위: defer
제목: benchmark baseline
상태: defer

### NX-421
우선순위: defer
제목: observability hardening
상태: defer
