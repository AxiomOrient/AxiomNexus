## 1. 현재 제품 한 줄 정의

현재 제품은 **goal file을 입력받아 로컬 workspace 안에서 단일 goal-run을 실행하고, `status / replay / resume / abort / doctor`로 그 실행 상태와 증거를 운영하는 single-agent CLI runtime**입니다. README, charter, capability matrix, runbook, autonomous target/spec가 모두 이 제품면을 중심으로 설명합니다.

---

## 2. 현재 설계 요약

### 현재 public surface

현재 문서와 CLI parser가 가리키는 정식 공개 표면은 `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help`입니다. README와 runbook이 이 명령 집합을 제품 사용법으로 제시하고, `cli_command.rs`의 usage/파서도 이 명령만 노출합니다. `resume`은 일반적인 재개가 아니라 `waiting_approval` 상태의 pending goal-run에 대한 승인 후 재개로 제한된다는 점도 charter와 runbook, 코드가 일치합니다.

### 현재 non-goal

현재 제품 계약상 non-goal은 멀티채널 메시징, daemon/service lifecycle, HTTP gateway, cron, skills marketplace, integrations catalog, benchmark/rehearsal automation, 넓은 agent platform 기능입니다. capability matrix에서도 browser/composio/delegate, long-term memory expansion, channels, gateway, daemon/service/cron/metrics HTTP를 experimental로 밀어냈습니다. 따라서 현재 제품은 개발/운영용 범용 플랫폼이 아니라 좁은 goal-run runtime입니다. ([GitHub][3])

### crate 책임 분리

`crates/core`는 최소 도메인 계약입니다. `RunGoal`, approval/budget/constraints, `DecisionOutcome`, `PolicyCode`, 최소 `AgentState`를 보유합니다. `crates/apps`는 CLI, goal-file 파싱, run orchestration, state/trace/doctor/status/replay/operator rendering, workspace lock, artifact/report 작성의 중심입니다. `crates/adapters`는 provider/tool/memory substrate 구현입니다. 이 분리는 charter 설명과 현재 트리, Cargo workspace 구성, 코드 위치가 일치합니다. ([GitHub][3])

### 상태 모델 / 실행 모델 / trace 모델 / artifact 모델 정합성

이 부분은 전반적으로 **좋습니다**.
상태 모델은 intentionally minimal합니다. `AgentState`는 revision, mode, 마지막 intent/actor/decision/policy만 저장하고, pending approval run은 `RuntimeStateSnapshot.pending_run`으로 별도 유지합니다. 실행 모델은 `RuntimeRunRecord`의 `phase / outcome / reason / verifier_state / artifacts`를 중심으로 잡혀 있고, trace는 JSONL 이벤트에 동일한 실행 의미와 artifact 인덱스를 기록합니다. `operator_render.rs`, `doctor.rs`, `status`, `replay` 출력도 같은 vocabulary를 재사용합니다. 즉 “core는 최소 상태”, “apps trace는 풍부한 실행 기록”이라는 역할 분리는 현재 제품 의미와 대체로 일치합니다. ([GitHub][4])

### 문서 계약과 코드 계약의 일치 정도

큰 방향은 일치합니다. README/charter/runbook/spec는 goal-file, approval, budget, terminal outcomes, replayable evidence를 말하고, 코드와 e2e는 실제로 `approval_required`, `budget_exhausted`, `operator_abort`, pending run, doctor/status/replay 출력, report artifact를 구현하고 검증합니다. 다만 세부적으로는 **검증 contract의 약화**, **이름/패키지 rename drift**, **constraints의 미집행**, **문서 truth lock 부족** 같은 어긋남이 남아 있습니다. ([GitHub][2])

### `00696de` 이후 구조 평가

`00696de`의 rename과 legacy 정리는 **방향 자체는 맞습니다**. 현재 public surface가 goal-run 중심으로 더 선명해졌고, parser도 좁아졌습니다. 다만 “제품 의미가 선명해졌는가” 기준으로 보면 rename은 아직 불완전합니다. 문서와 테스트는 `AxiomRunner`/`AXIOMRUNNER_*`/`axiomrunner_apps`를 말하지만 Cargo 패키지명은 여전히 `axonrunner_*`입니다. 또 public surface에서는 legacy가 줄었지만, planner/history/doc 계층에는 legacy 잔재가 남아 있습니다. 즉 정리는 맞았지만 truth lock이 끝난 상태는 아닙니다. ([GitHub][1])

---

## 3. blocker findings

### 문제 1. 기본 goal verifier가 placeholder라서 “verify-before-done” 계약을 실제로 잠그지 못함

* **위치**: `crates/apps/src/runtime_compose/plan.rs`, `docs/AUTONOMOUS_AGENT_SPEC.md`, `docs/AUTONOMOUS_AGENT_TARGET.md`
* **왜 문제인지**: 현재 spec은 verification checks와 done condition을 제품 계약의 핵심으로 두고, verify-before-done을 원칙으로 둡니다. 그런데 explicit workflow pack이 없을 때 생성되는 기본 workflow pack의 verifier rule은 `command_example = "pwd"`이고, 테스트도 이 placeholder를 고정합니다. 이는 “검증이 실제 요구사항 충족을 증명한다”는 현재 제품 약속을 약화시킵니다. ([GitHub][5])
* **blocker 여부**: **예**. 현재 제품이 자율 goal-run runtime이라고 주장하려면 기본 goal path에서도 검증이 placeholder여서는 안 됩니다. ([GitHub][2])
* **최소 수정안**: explicit workflow pack이 없으면 `pwd` 같은 placeholder를 쓰지 말고 `verification_checks`를 `weak/unresolved`로 노출하면서 성공 terminal outcome을 막거나, 기본 pack을 `pack_required`로 처리해 명시적 verifier가 없으면 `blocked`로 종료합니다. verifier strength는 이미 workflow pack contract에 vocabulary가 있습니다. ([GitHub][6])
* **더 좋은 구조적 해법**: representative app/server workflow pack을 first-class로 두고, 기본 goal path는 “generic autonomous runtime”이 아니라 “verifier profile 미지정 시 완료 불가”로 설계합니다. 즉 `done`의 의미를 verifier contract가 보장하게 만듭니다. ([GitHub][5])

### 문제 2. rename 이후 제품 이름/바이너리/env 계약이 하나로 잠기지 않음

* **위치**: `00696de` 커밋, README, tests/nightly scripts, Cargo package names
* **왜 문제인지**: 커밋은 “AxonRunner → AxiomRunner” rename을 선언했지만, Cargo 패키지명은 `axonrunner_apps/core/adapters`이고, README/tests/nightly는 `axiomrunner_apps`와 `AXIOMRUNNER_*`를 사용합니다. 제품 이름, 바이너리 이름, 환경변수 prefix가 다층적으로 어긋나면 operator 문서, build output, CI, release packaging이 같은 제품을 말하지 않게 됩니다. ([GitHub][1])
* **blocker 여부**: **예**. release 전 truth surface가 하나로 잠겨야 합니다. ([GitHub][7])
* **최소 수정안**: 제품 공식 이름, 바이너리 이름, env prefix를 하나로 선택하고 Cargo package/bin, README, tests, scripts, doctor 출력, runbook를 모두 일치시킵니다.
* **더 좋은 구조적 해법**: release gate에 “이름/바이너리/env prefix 단일성” 검사를 추가해 drift를 자동 차단합니다. 현재 gate는 retained command token 존재만 주로 검사합니다. ([GitHub][7])

### 문제 3. `constraints`가 goal schema의 일부인데 runtime에서 실제 집행되지 않음

* **위치**: `crates/core/src/intent.rs`, `docs/AUTONOMOUS_AGENT_SPEC.md`
* **왜 문제인지**: public spec은 `constraints`를 explicit non-goals, compatibility rules, safety limits로 설명합니다. 그런데 코드 주석은 이 제약이 현재는 validate되고 artifact에 기록될 뿐 runtime에서 집행되지 않는다고 명시합니다. 즉 사용자에게는 safety limit처럼 보이지만 실제로는 advisory metadata에 가깝습니다. ([GitHub][8])
* **blocker 여부**: **예**. 현재 제품이 workspace-bound autonomous runtime이라면 goal-level safety constraint는 최소한 일부라도 실제 정책으로 이어져야 합니다. ([GitHub][5])
* **최소 수정안**: public docs에 현재 constraints가 “validated + recorded, not enforced”임을 명시하고, status/report에 enforcement mode를 노출합니다.
* **더 좋은 구조적 해법**: constraints를 tool allowlist / approval policy / verifier gating에 연결해 최소 enforcement subset을 구현합니다. 예를 들어 destructive commands, path scope, external network/tool class를 constraints와 policy code로 연결합니다. ([GitHub][6])

### 문제 4. release truth lock이 약해서 stale contract가 남아도 gate를 통과할 수 있음

* **위치**: `crates/apps/tests/release_security_gate.rs`, `CHANGELOG.md`, `docs/WORKFLOW_PACK_CONTRACT.md`, `docs/transition/WORKFLOW_PACK_CONTRACT.md`
* **왜 문제인지**: visible release gate는 README/charter/runbook/spec 등에 retained command 토큰이 존재하는지 확인합니다. 하지만 CHANGELOG는 여전히 `batch`와 legacy aliases를 kept surface처럼 말하고, workflow pack contract는 root docs와 transition docs에 중복 존재합니다. 즉 “현재 truth가 하나인지”보다는 “필수 단어가 있나” 정도만 잠그고 있습니다. ([GitHub][7])
* **blocker 여부**: **예**. 지금 단계는 하드닝/릴리즈 직전 단계라 문서 truth drift 자체가 운영 리스크입니다.
* **최소 수정안**: stale command/binary/env/contract token의 **부재**도 gate에서 검사하고, authoritative contract 문서를 하나만 남깁니다.
* **더 좋은 구조적 해법**: “current truth docs allowlist”와 “forbidden legacy tokens”를 별도 테스트로 고정합니다. transition docs는 링크 브리지로만 남기고 내용 계약은 루트 docs 한 벌에만 둡니다. ([GitHub][9])

---

## 4. non-blocking findings

### 문제 1. public surface에서는 legacy가 줄었지만 internal model에는 legacy 경로가 남아 있음

* **위치**: `crates/apps/src/cli_command.rs`, `crates/apps/src/runtime_compose/plan.rs`, `CHANGELOG.md`
* **왜 문제인지**: parser는 현재 정식 명령만 노출하지만, planner 계층과 history 문서에는 legacy intent/compatibility surface의 흔적이 남아 있습니다. public contract를 깨는 직접 증거는 아니지만, 내부 분기가 많아질수록 product meaning drift가 생기기 쉽습니다. ([GitHub][10])
* **blocker 여부**: 아니오. 현재는 architecture cleanup 성격입니다.
* **최소 수정안**: public로 노출되지 않는 legacy path를 `compat` 모듈로 격리하거나 dead-path 여부를 테스트로 확인합니다.
* **더 좋은 구조적 해법**: goal-file path 외 compose/plan branches를 제거하고, legacy migration은 입력 변환 layer 한 곳에서만 처리합니다.

### 문제 2. workflow pack contract 문서가 두 군데에 남아 있어 drift 가능성이 있음

* **위치**: `docs/WORKFLOW_PACK_CONTRACT.md`, `docs/transition/WORKFLOW_PACK_CONTRACT.md`, `docs/DOCS_ALIGNMENT.md`
* **왜 문제인지**: root docs가 current truth이고 transition docs는 브리지라는 설명이 있지만, 실제로는 같은 계약 문서가 두 벌 있습니다. 내용 차이가 작더라도 계약 문서 이중화는 시간이 지나면 높은 확률로 drift를 만듭니다. ([GitHub][9])
* **blocker 여부**: 아니오. 하지만 문서 truth lock 관점에서는 조기 정리가 좋습니다.
* **최소 수정안**: transition 문서는 root 문서 링크만 남기고 본문 계약을 제거합니다.
* **더 좋은 구조적 해법**: docs tree에 “authoritative/current”와 “historical/bridge”를 물리적으로 분리합니다.

### 문제 3. trace/replay는 의미가 잘 맞지만 durability/scale 전략은 아직 단순함

* **위치**: `crates/apps/src/trace_store.rs`, `crates/apps/src/state_store.rs`
* **왜 문제인지**: state snapshot은 temp+rename으로 비교적 안전하지만, trace는 JSONL append와 full reload 중심입니다. 현재 제품 계약을 즉시 깨는 증거는 없고 replay/summary도 잘 작동하지만, 장기적으로 대형 run/다중 프로세스 contention에는 더 강한 전략이 필요할 수 있습니다. ([GitHub][11])
* **blocker 여부**: 아니오. 현재 단계에서는 later/observability 성격입니다.
* **최소 수정안**: single-writer contract를 runbook/doctor에 명시하고 stress test를 추가합니다.
* **더 좋은 구조적 해법**: append-only event log + bounded index/compaction 전략을 별도 모듈로 분리합니다.

### 문제 4. release gate는 존재하지만 “무엇을 잠그는가”가 아직 충분히 공격적이지 않음

* **위치**: `release_security_gate.rs`, `nightly_dogfood_contract.rs`, `autonomous_eval_corpus.rs`
* **왜 문제인지**: 현재 테스트는 retained command presence, autonomy tokens, nightly dogfood summary, representative eval corpus를 잘 검사합니다. 다만 “placeholder verifier 금지”, “stale legacy surface 금지”, “naming truth lock” 같은 부정적 invariant는 아직 명시적으로 잠그지 않습니다. ([GitHub][7])
* **blocker 여부**: 아니오. 하지만 must-fix release hardening에 가깝습니다.
* **최소 수정안**: forbidden token / forbidden placeholder / identity consistency / verifier strength assertions를 추가합니다.
* **더 좋은 구조적 해법**: release gate를 “truth lock suite + safety suite + autonomy suite”로 분리합니다.

---

## 5. 추천 로드맵

### A. must-fix before release

#### 1) identity truth lock

* **왜 중요한지**: 제품 이름, 바이너리, env prefix가 다르면 operator 문서와 실제 실행 경로가 갈라집니다. ([GitHub][1])
* **줄이는 위험**: release packaging 오류, CI/env 설정 혼선, 문서-실행 불일치.
* **완료 증거**: Cargo/bin/env/docs/tests/release gate가 하나의 이름만 사용하고, 다른 이름이 forbidden token 테스트에서 걸립니다.

#### 2) verification contract lock

* **왜 중요한지**: 현재 제품의 핵심은 goal-run을 “검증된 완료”로 끝내는 것입니다. `pwd` placeholder는 이 약속을 약화시킵니다. ([GitHub][5])
* **줄이는 위험**: false done, false success, 형식적 성공.
* **완료 증거**: default goal path에서 placeholder verifier가 사라지고, 명시적 pack 또는 unresolved/blocked semantics가 e2e와 eval에서 검증됩니다.

#### 3) constraints contract lock

* **왜 중요한지**: safety limits가 실제 enforce되지 않으면 goal schema가 operator에게 과도한 신뢰를 유발합니다. ([GitHub][8])
* **줄이는 위험**: 안전 기대와 실제 실행의 괴리.
* **완료 증거**: public docs에 advisory/enforced 범위가 명확해지고, 최소 enforcement subset이 policy/tool layer에 연결됩니다.

#### 4) docs/release gate truth lock

* **왜 중요한지**: 지금은 stale legacy/duplicate contract가 남아도 gate 통과 가능성이 있습니다. ([GitHub][7])
* **줄이는 위험**: release 후 문서 drift, support 비용 증가.
* **완료 증거**: authoritative docs 한 벌, forbidden legacy tokens 테스트, transition docs 브리지화.

### B. architecture cleanup

#### 1) internal legacy pruning

* **왜 중요한지**: public surface는 이미 좁아졌으므로 내부 분기도 그 의미에 맞춰 줄여야 합니다. ([GitHub][10])
* **줄이는 위험**: 숨은 compatibility branch, 이후 리팩터링 비용.
* **완료 증거**: compose/plan 경로에서 current goal-run 외 호환 분기가 축소되고, migration layer 한 곳만 남음.

#### 2) apps 내부 책임 분리

* **왜 중요한지**: 현재 `apps`가 lifecycle/orchestration/rendering/state/trace를 모두 잡고 있습니다. 제품 커질수록 blast radius가 큽니다. ([GitHub][12])
* **줄이는 위험**: 변경 시 회귀 범위 확대.
* **완료 증거**: lifecycle control, artifact/report assembly, rendering이 내부 모듈 경계로 분리.

### C. operator experience / observability

#### 1) verifier strength / constraint enforcement visibility

* **왜 중요한지**: operator는 “성공”과 “검증 강도”를 구분해서 봐야 합니다. workflow pack contract 자체가 weak/unresolved semantics를 이미 정의합니다. ([GitHub][6])
* **줄이는 위험**: operator가 weak verification을 true success로 오해.
* **완료 증거**: status/replay/doctor/report에 verifier strength, constraint enforcement mode, approval reason이 명시됨.

#### 2) trace reliability hardening

* **왜 중요한지**: replay는 이 제품의 핵심 운영면입니다. ([GitHub][13])
* **줄이는 위험**: 장시간 run 분석 실패, 디버깅 비용 증가.
* **완료 증거**: append/store stress tests, corruption handling, artifact index invariants가 테스트로 잠김.

### D. later / optional

#### 1) richer on-risk heuristics

현재 `OnRisk`는 사실상 `Always`에 가깝게 보수적으로 동작합니다. 이는 지금 단계에서는 괜찮고, 나중에 risk heuristics가 필요할 때 개선하면 됩니다. ([GitHub][14])

#### 2) long-term memory / channels / gateway / browser tools

현재 charter와 capability matrix 기준으로는 제품면 밖입니다. 지금 건드리면 제품 의미를 다시 흐립니다. ([GitHub][3])

### 가장 짧은 critical path

#### 단계 1. Identity & truth lock

* **목표**: 제품 이름, 바이너리, env prefix, docs, tests를 하나로 잠금.
* **수정 대상**: Cargo manifests, README, runbook, tests, scripts, release gate.
* **완료 조건**: 하나의 이름만 남고 forbidden old-name token이 gate에서 차단됨.
* **검증 방법**: release gate + nightly + CLI smoke.
* **선행 조건**: 없음.

#### 단계 2. Verification contract lock

* **목표**: default goal path에서 placeholder verifier 제거.
* **수정 대상**: `runtime_compose/plan.rs`, workflow pack docs, e2e/eval tests.
* **완료 조건**: explicit verifier 없이는 `done` 불가, 또는 weak/unresolved가 명시적으로 surfaced.
* **검증 방법**: e2e + autonomous eval corpus + replay artifact assertions.
* **선행 조건**: 단계 1.

#### 단계 3. Constraint/safety contract lock

* **목표**: constraints의 실제 의미를 문서와 코드에서 일치시킴.
* **수정 대상**: `core::intent`, policy/tool enforcement path, README/spec/runbook/status/report.
* **완료 조건**: advisory/enforced 범위가 명확하고 최소 enforcement subset이 작동.
* **검증 방법**: negative tests for forbidden tools/paths/commands.
* **선행 조건**: 단계 2.

#### 단계 4. Legacy pruning & doc consolidation

* **목표**: public contract 밖의 legacy와 duplicated contract 문서 정리.
* **수정 대상**: planner/compose compat branches, CHANGELOG, transition docs.
* **완료 조건**: current truth docs 한 벌, internal compat 분기 축소.
* **검증 방법**: release gate + grep-based forbidden surface tests.
* **선행 조건**: 단계 1~3.

#### 단계 5. Observability/reliability hardening

* **목표**: operator가 verifier strength와 run evidence를 더 정확히 읽게 만들고 trace/store 신뢰성 강화.
* **수정 대상**: operator_render, doctor, status, replay, trace tests.
* **완료 조건**: weak verification/constraint mode/rollback/checkpoint가 명확히 표시되고 stress tests가 green.
* **검증 방법**: nightly dogfood + replay integrity tests.
* **선행 조건**: 단계 2.

### 지금 건드리면 안 되는 영역

지금은 channels, daemon/service, HTTP gateway, browser/composio/delegate, multi-agent orchestration, long-term memory expansion을 건드리지 않는 편이 맞습니다. 현재 charter/capability matrix가 이들을 제품 밖으로 밀어냈기 때문입니다. ([GitHub][3])

---

## 6. 가장 좋은 다음 한 수

가장 좋은 다음 한 수는 **“identity + truth lock PR”을 먼저 내는 것**입니다.

이 PR의 범위는 세 가지면 충분합니다.

1. 공식 제품 이름 / 바이너리 이름 / env prefix를 하나로 확정
2. CHANGELOG와 duplicated workflow-pack docs 정리
3. release gate에 forbidden legacy/naming drift 검사를 추가

이걸 먼저 해야 이후 verifier/constraints 작업도 “어느 제품을 위한 계약인지”가 흔들리지 않습니다. 