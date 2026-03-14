## 결론

> **방향은 맞다. 하지만 아직 “최종 청사진대로 닫힌 구현”은 아니다.**

즉,

- **개념적 방향**: 맞음
- **문서 방향**: 대체로 맞음
- **실제 빌드/실행 타깃 정합성**: 아직 불안정
- **`WorkspacePort` 제거**: 아직 미완료
- **`RuntimePort::execute_turn` 최종 계약 구현**: 아직 미완료
- **다음 작업 계획**: 지금 바로 재정렬이 필요

현재 README와 spec은 AxiomNexus를 IDC control plane, Surreal-first, triad external companion, `RuntimePort::execute_turn`, append-only `TransitionRecord` 중심으로 설명합니다. 그런데 실제 Cargo 기준 빌드 주체는 여전히 루트 패키지가 아니라 `crates/axiomsync` 쪽이고, `WorkspacePort`도 코드에 남아 있습니다. 그래서 **문서는 최종형을 말하는데, 저장소의 실제 실행물은 과도기 상태**입니다. ([GitHub][2])

이번 검토 기록은 임시 감사 패키지로 만들었지만, 현재 저장소 기준으로는 더 이상 canonical 문서가 아닙니다.
이후 기준 문서는 `README.md`, `docs/*`, `samples/*`, `src/*`입니다.

---

## 근거

### 2) 문서가 말하는 최종 방향은 좋다

현재 README와 `docs/spec` / `docs/adr`는 다음을 명확히 선언합니다.

- IDC only write path
- Surreal-first, PostgreSQL-later
- triad external companion
- `WorkspacePort` 제거
- `RuntimePort::execute_turn`
- Store semantic contract / adapter conformance

이 방향 자체는 단순하고 본질적이며, 이전전 답변의 최종 설계와도 일치합니다. ([GitHub][2])

### 3) 하지만 실제 빌드 타깃은 아직 AxiomNexus로 닫히지 않았다

루트 `Cargo.toml`은 현재 `[workspace]`만 있고 member가 `crates/axiomsync` 하나입니다. 즉 루트 `src/main.rs`, `src/lib.rs`가 있어도 **루트가 실제 패키지로 빌드되는 구조가 아닐 수 있습니다**. 반면 `crates/axiomsync`는 여전히 별도 crate이며 README/Cargo 설명도 AxiomSync, `axiom://`, SQLite retrieval runtime 중심입니다. 이건 현재 가장 큰 정합성 문제입니다. ([GitHub][3])

### 4) `WorkspacePort` 제거는 아직 문서에만 있다

문서와 ADR은 `WorkspacePort` 제거를 명시하지만, 실제 코드에는 `src/port/workspace.rs`가 있고 `src/port/mod.rs`도 이를 export합니다. `run_turn_once` 역시 아직 `workspace: &impl WorkspacePort`를 인자로 받습니다. 즉, 핵심 경계 변경이 아직 실행 경로에 반영되지 않았습니다. ([GitHub][4])

### 5) `execute_turn` 최종 계약도 아직 덜 구현됐다

`samples/execute-turn-output.schema.json`은 `observations`를 필수로 요구합니다. 그런데 실제 `src/port/runtime.rs`의 `ExecuteTurnOutcome`에는 `observations`가 없고, coclai runtime adapter도 이를 채우지 않습니다. 즉, schema와 Rust 타입/adapter가 drift 상태입니다. ([GitHub][5])

### 6) 계획 문서의 done 표시는 과대하다

`plans/IMPLEMENTATION-PLAN.md`와 `plans/TASKS.md`는 여러 항목을 done으로 기록하지만, 실제 코드 기준으로는 `WorkspacePort` 제거, runtime observation 수렴, authoritative build target 통일이 아직 안 끝났습니다. 그래서 다음 단계는 “새 기능 추가”보다 **정합성 회복**이 먼저입니다. ([GitHub][6])

---

## 지금 올바른 방향으로 가고 있는가?

**반은 맞고, 반은 아직 아닙니다.**

정확히 말하면:

- **최종 목적지**는 맞습니다.
- **그 목적지로 가는 저장소 구조**는 아직 덜 정리됐습니다.

한 문장으로 요약하면:

> **AxiomNexus는 올바른 철학을 갖고 있지만, 아직 “문서가 말하는 제품”과 “Cargo가 빌드하는 제품”이 하나가 아니다.**

이건 위험 신호입니다.
지금 여기서 바로잡지 않으면 이후 커밋이 계속 쌓여도 핵심 불일치가 커집니다.

---

## 다음 계획

당시 정리한 구현 우선순위는 아래 순서였습니다.

### Phase 0 — 감사 기준선 고정

현재 상태를 “끝난 것”으로 취급하지 말고, 감사 결과를 새 기준선으로 고정합니다.

### Phase 1 — authoritative build target 통일

루트 AxiomNexus가 실제 빌드/실행 타깃이 되게 바꿔야 합니다.
이 단계가 끝나야 README, scripts, docs, `src/*`가 모두 같은 제품을 설명하게 됩니다. ([GitHub][3])

### Phase 2 — `WorkspacePort` 제거

`src/port/workspace.rs` 제거, `run_turn_once` 리팩터링, 테스트 갱신까지 한 번에 해야 합니다. ([GitHub][4])

### Phase 3 — runtime observation 폐루프

`ExecuteTurnOutcome.observations`를 실제 타입과 adapter에 넣고, schema와 테스트를 맞춥니다. ([GitHub][5])

### Phase 4 — Store semantic closure

Surreal adapter를 semantic contract 기준으로 잠그고, conformance suite를 추가합니다. 이 단계가 끝나야 PostgreSQL-later 전략이 진짜가 됩니다. ([GitHub][7])

### Phase 5 — legacy `axiomsync` 정리

active reader path에서 내립니다. archive거나 migration bridge여야지, active product처럼 보여서는 안 됩니다. ([GitHub][8])

### Phase 6 — hardening

replay, schema, no-workspace-port, smoke assertions를 gate로 잠급니다.

---

## 10개 이상 커밋으로 바로 실행 가능한 내역

당시 정리한 커밋 계획의 핵심만 추리면:

1. audit baseline 고정
2. implementation plan / tasks 재작성
3. root `axiomnexus`를 authoritative cargo package로 전환
4. scripts를 실제 build target에 맞게 수정
5. metadata를 AxiomNexus 기준으로 정렬
6. `WorkspacePort` 제거
7. `ExecuteTurnReq.gate_plan` 추가
8. `ExecuteTurnOutcome.observations` 추가
9. coclai runtime adapter가 observations emit
10. `run_turn_once`를 workspace-free로 리팩터링
11. runtime observation fixtures 기반 테스트 교체
12. StorePort role traits 정리
13. Surreal adapter의 `commit_decision` semantic transaction 정렬
14. adapter conformance suite 추가
15. README와 target architecture를 실제 코드 상태에 맞게 재정렬
16. legacy `axiomsync`를 active surface에서 분리
17. no-workspace-port invariant + schema conformance test 추가
18. smoke runtime assertions 강화

---

## 재현/실행 방법

현재는 아래 순서로 보면 됩니다.

1. [README.md](./README.md)
2. [문서 인덱스](./docs/00-index.md)
3. [최종 도착지](./docs/01-FINAL-TARGET.md)
4. [청사진](./docs/02-BLUEPRINT.md)
5. [품질 게이트](./docs/05-QUALITY-GATES.md)
