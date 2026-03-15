# IMPLEMENTATION-PLAN.md

# AxiomNexus 구현 계획

## 0. 한 문장 정의

AxiomNexus는 **AI가 수행하는 소프트웨어 팀의 업무를 작업 단위로 운영하고, 각 상태 전이를 계약과 증거로 판정하고 기록하는 control plane**이다.

즉, 이 프로젝트는 둘 중 하나가 아니다.

- Paperclip 같은 광범위한 회사 운영 OS
- 단순한 코드 변경 승인기

정확한 위치는 이 사이에 있다.

> **작업을 잃지 않는 AI 소프트웨어 팀 운영 커널**

---

## 1. 무엇을 만드는가

현재 저장소가 이미 노출하는 운영 표면은 다음이다.

- company
- contract
- agent
- work
- run
- activity
- queue / wake / replay / export / import

이 표면은 유지한다.
다만 복잡도의 중심은 오직 `work transition kernel` 에 둔다.

즉 제품은 다음 두 층으로 이해해야 한다.

### A. 운영 표면
운영자가 팀과 작업을 굴리는 표면

- 회사/계약 설정
- 에이전트 등록/일시중지/재개
- work 생성/수정/queue/wake/reopen/cancel/override
- run 관찰
- activity / consumption 관찰
- replay / export / import

### B. 핵심 커널
실제 권한과 정합성을 보장하는 중심

Intent → Decide → Commit

이 커널이 없으면 운영 표면은 단순 task manager가 되고,
운영 표면이 없으면 커널은 단순 “심판 + 기록관”으로만 보인다.

AxiomNexus는 **운영 표면을 가진 업무 control plane**이어야 한다.
단, 회사 운영 OS처럼 goals/budgets/org chart까지 확장하지는 않는다.

---

## 2. 제품 범위 고정

## 포함

- contract-first IDC kernel
- append-only TransitionRecord
- Work / Lease / Wake / Session / Run / Activity / Consumption
- operator CLI: serve / scheduler once / run once / replay / export / import
- HTTP read/write surface for work/run/activity/agent/company/contract
- coclai runtime 하나
- Surreal-first store
- preview/dogfood 운영 가능 상태

## 제외

- goals / budgets / org chart / membership / auth ecosystem 확대
- multi-runtime generalization
- workflow builder DSL
- repo-local triad embedding
- PostgreSQL work (이번 버전 제외)

---

## 3. 지금 어디까지 왔는가

현재 저장소 기준으로 이미 정리된 축:

- root package = `axiomnexus`
- `WorkspacePort` 제거
- `RuntimePort::execute_turn`
- `observations` 기반 runtime turn
- canonical operator path로 `scheduler once`
- diagnostic path로 `run once <run_id>`
- SurrealKV 기반 preview-ready control plane

즉, 아키텍처 재설계 단계는 끝났다.
지금부터는 **실제 사용 가능성**과 **운영 정합성**을 닫는 단계다.

---

## 4. 이제 무엇을 완성해야 하는가

이번 버전에서 남은 핵심은 4개뿐이다.

## Phase 1 — 제품 정체성 잠금

### 목표
README와 실제 운영 언어를 하나로 만든다.

### 왜 필요한가
지금 가장 큰 위험은 기술이 아니라 scope drift다.
Paperclip를 좇으면 범위가 커지고,
반대로 “심판 + 기록관”으로만 축소하면 제품 가치가 사라진다.

### 작업
- README의 한 문장 정의를 “AI 소프트웨어 팀 업무 control plane”으로 고정
- quick start와 명령어 역할을 운영 관점으로 다시 설명
- `scheduler once`와 `run once` 역할을 명확히 분리
- “무엇을 하지 않는가”를 짧게 명시

### 완료 조건
새 사용자가 README만 읽고도
“이건 회사 OS가 아니라, AI 팀의 work/run control plane이구나”를 이해한다.

---

## Phase 2 — 실제 사용 흐름 완성

### 목표
이 시스템을 실제로 어떻게 쓰는지 한 번에 보이게 만든다.

### 작업
- canonical preview workflow를 하나 고정
- 최소 데모 흐름 문서화:
  1. 회사 생성
  2. 계약 활성화
  3. agent 등록
  4. work 생성
  5. queue
  6. `scheduler once`
  7. accepted transition 확인
  8. activity / replay 확인
- smoke script도 이 흐름을 그대로 검증하게 맞춤

### 완료 조건
“언제 어떻게 쓰는가?”에 대한 대답이 명확하다.
내부 팀이 실제로 preview 운영을 시작할 수 있다.

---

## Phase 3 — release evidence를 실제 운영 기준으로 강화

### 목표
release checklist와 실제 검증 스크립트가 같은 것을 말하게 만든다.

### 작업
smoke/verify에서 아래를 직접 확인:
- accepted transition
- `TransitionRecord` append
- `WorkSnapshot.rev` 증가
- run completed
- task_session 저장/갱신
- consumption 기록
- replay pass

### 완료 조건
체크리스트에 적힌 항목을 스크립트가 그대로 검증한다.

---

## Phase 4 — preview release

### 목표
내부 dogfood에 실제로 올린다.

### 사용 방식
- 운영자는 `serve`를 띄운다
- queued work를 `scheduler once`로 처리한다
- 필요 시 `run once <run_id>`로 재현한다
- activity / replay / export 로 운영 상태를 확인한다

### 완료 조건
이 시스템이 “설계물”이 아니라 “실제로 작업을 흘려보내는 도구”가 된다.

---

## 5. 이번 버전에서 하지 않을 것

이번 버전에서 아래는 하지 않는다.

- PostgreSQL adapter
- dual-store conformance
- benchmark baseline
- 회사 운영 제품 면 확장
- 범용 workflow builder
- 복수 runtime adapter
- triad repo-local integration

이것들은 stable 이후 논의한다.

---

## 6. 가장 짧은 성공 기준

이번 버전이 끝났다고 말하려면 아래가 성립해야 한다.

1. 팀이 실제로 work를 queue 하고 처리할 수 있다.
2. `scheduler once`가 canonical operator path로 설명되고 사용된다.
3. accepted transition이 record / snapshot / session / consumption까지 남는다.
4. replay가 현재 상태를 검증한다.
5. 운영자가 activity와 run을 보고 “무슨 일이 있었는지” 이해할 수 있다.

여기까지가 이번 버전의 끝이다.
