# 설계 검토와 자기 피드백

이 문서는 이번 패키지를 만들기 전에 **이전 답변의 흔들린 부분을 스스로 점검하고 수정한 결과**를 기록한다.

---

## 1. 무엇이 문제였는가

### 문제 A — Store backend와 runtime boundary를 섞었다
이전 문서에서는 PostgreSQL 논의를 하면서 runtime boundary까지 보수적으로 흔들렸다.  
하지만 현재 저장소 기준으로 이 둘은 다른 축이다.

- store backend: Surreal-first / PostgreSQL-later
- runtime boundary: `WorkspacePort` 제거, `RuntimePort::execute_turn` 수렴

현재 저장소의 목표 아키텍처 문서는 `RuntimePort`를 `execute_turn` 하나로 좁히는 방향을 명시하고 있다. [R2]  
이번 리팩터링으로 `workspace` 포트는 제거했고, 그 책임은 `execute_turn`의 gate plan / observations contract로 흡수했다. [R4][R5]  
즉 local observation은 이제 과도기 표면이 아니라 runtime turn 경계 안에서 닫힌다.

### 문제 B — 문서 세트가 MECE하지 않았다
이전 패키지는 개요 수준 문서만 있었고, 아래 항목이 분리되지 않았다.

- 왜 이런 결정을 했는가
- 최종 도착지는 무엇인가
- 구현 불변식은 무엇인가
- adapter portability를 무엇으로 보장하는가
- 지금 당장 무엇부터 구현해야 하는가

이 패키지에서는 이를 `docs/`, `spec/`, `adr/` 로 분리했고, delivery-only 계획 문서는 ship surface에서 제외했다.

### 문제 C — StorePort 의미론이 부족했다
Surreal과 PostgreSQL을 바꿔 끼우려면 CRUD 추상화가 아니라 **semantic contract**가 있어야 한다.  
현재 저장소도 `StorePort`를 aggregate surface로 두고, call site는 narrower role traits에 의존하게 되어 있다. [R2][R6]  
따라서 final package는 `StorePort`의 operation semantics와 conformance suite를 핵심 문서로 승격했다.

### 문제 D — Runtime 결과와 kernel 판정을 충분히 분리하지 못했다
현재 runtime 포트는 이미 `execute_turn`을 갖고 있고, `TransitionIntent`, raw output, usage, invalid-session 여부를 반환한다. [R4]  
이번 리팩터링은 여기에 changed files / command results / artifact refs를 observations로 수렴시킨다. [R5]

- runtime은 관측 결과를 반환한다.
- kernel은 그 관측 결과를 evidence로 해석한다.
- gate verdict는 runtime이 아니라 kernel이 만든다.

이렇게 해야 `RuntimePort`가 커져도 policy engine으로 변질되지 않는다.

---

## 2. 무엇을 유지했는가

다음은 **유지**했다.

1. single-crate modular monolith [R2]
2. `Intent -> Decide -> Commit` 단일 write path [R1][R2][R3]
3. `TransitionRecord` append-only ledger [R1][R2][R7]
4. triad external companion [R1][T1]
5. 개발 중 Surreal-first [R1]
6. 나중 PostgreSQL adapter 추가 가능성 [S1][PG1][PG2]

---

## 3. 무엇을 바꿨는가

### 변경 1 — `WorkspacePort` 제거를 최종 결정으로 승격
이전엔 설명 수준이었지만, 이번엔 아예 ADR과 spec로 고정했다. [R2][R4][R5]

### 변경 2 — `execute_turn` 결과를 명시적으로 확장
현재 runtime 결과에 아래를 추가하는 최종형을 제안한다.

- changed files
- command results
- artifact refs
- optional notes

이렇게 하면 workspace 관측이 runtime turn 안에서 닫힌다.

### 변경 3 — adapter portability를 `StorePort + Conformance Suite + Export/Replay`로 정의
DB를 바꿔 끼우는 핵심은 generic query abstraction이 아니다.  
핵심은:

- semantic contract가 같아야 하고
- conformance tests를 통과해야 하고
- export/import/replay로 상태를 옮길 수 있어야 한다

이다. [S2][PG1][PG2][PG3]

---

## 4. 최종 판단

이번 패키지의 최종 판단은 아래 한 문장으로 요약된다.

> **AxiomNexus final은 `WorkspacePort`를 제거한 단일 runtime turn 경계를 가지며, 개발 중에는 SurrealDB/SurrealKV로 닫고, 의미론을 고정한 뒤 PostgreSQL adapter를 추가하는 구조가 가장 단순하고 본질적이다.**

이 판단은 현재 저장소가 이미 선언한 방향과도 가장 잘 맞는다. [R1][R2][R4][R5]
