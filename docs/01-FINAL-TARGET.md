# 최종 도착지

## 한 문장 정의

**AxiomNexus는 회사 소유 contract가 work 상태 전이를 판정하고, append-only `TransitionRecord` ledger에 그 결과를 남기는 Rust 기반 IDC control plane이다.** [R1][R2][R3]

---

## 제품의 본질

AxiomNexus의 본질은 “에이전트가 일한다”가 아니다.  
본질은 **“에이전트가 제출한 intent가 계약에 의해 판정되고, 그 판정이 ledger에 영속화된다”** 이다. [R1][R2]

따라서 최종 제품은 아래 다섯 질문에 강해야 한다.

1. 지금 어떤 work가 누구 손에 있나?
2. 왜 이 상태 전이가 허용되었나 / 거부되었나?
3. wake와 session은 왜 이렇게 정리되었나?
4. 지금 상태를 `TransitionRecord`로 재구성할 수 있나?
5. 같은 규칙을 다른 store adapter에서도 유지할 수 있나?

---

## 범위

### 반드시 포함
- single-crate modular monolith [R2]
- `Intent -> Decide -> Commit` 단일 write path [R1][R2]
- `TransitionRecord` append-only ledger [R1][R7]
- `WorkLease` exclusive control
- `PendingWake` coalescing
- `TaskSession` continuity [R8]
- `RuntimePort::execute_turn`
- Surreal-first dev store [R1][S4]
- PostgreSQL-later adapter [S1][PG1][PG2]
- triad external verification companion [R1][T1]
- replay / export / import
- axum 기반 HTTP / SSE 운영 표면 [AX1][AX2]

### 유지하되 확장하지 않을 것
- company / contract / agent / work / run / activity 운영 표면 [R1][R3]
- coclai 단일 runtime 전제 [R1][R2]

### 제외
- broad company OS surface
- goals / budgets / org chart 확대
- multi-runtime 일반화
- plugin system
- policy DSL / OPA / CUE
- repo-local triad workspace
- generic repository/service layering

---

## 최종 이름과 canonical assets

### 이름
- 제품명: `AxiomNexus`
- package/bin: `axiomnexus`
- env prefix: `AXIOMNEXUS_`
- local data dir: `.axiomnexus/`

### canonical assets
- repo-wide rule: `AGENTS.md` [R1]
- runtime prompt policy: `.agents/AGENTS.md` [R1]
- transition executor skill: `.agents/skills/transition-executor/SKILL.md` [R1]
- runtime intent schema: `samples/transition-intent.schema.json` [R1][R10]
- runtime execute-turn schema: `samples/execute-turn-output.schema.json` *(이번 패키지에서 추가)*

---

## 최종 성공 조건

최종 버전이 완성되었다고 말할 수 있으려면 아래가 모두 성립해야 한다.

1. 모든 상태 변경이 `TransitionRecord`로 설명된다. [R2]
2. `commit_decision`이 lease와 `expected_rev`를 원자적으로 검증한다. [R2][R6]
3. `WorkspacePort` 없이 runtime turn이 commit 직전까지 필요한 관측을 모두 수집한다. [R4][R5]
4. store adapter를 바꿔도 `StorePort` conformance suite가 동일하게 통과한다.
5. replay가 snapshot을 재구성한다. [R1]
6. triad 결과는 decision을 돕지만, AxiomNexus state를 직접 mutate하지 못한다. [T1]

---

## 최종 목적지를 가장 짧게 표현하면

> **AxiomNexus는 “에이전트 실행기”가 아니라 “계약 기반 상태 전이 커널”이다.**

이 문장을 흔들지 않으면 구조가 흔들리지 않는다.
