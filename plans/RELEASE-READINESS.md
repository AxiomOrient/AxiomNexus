# Release Readiness

## 최종 판단

### 1. 지금 바로 출시해도 되는가?

#### 내부 preview / dogfood
**조건부 yes**

아래 3개만 먼저 닫으면 내부 배포는 가능하다.

1. canonical docs 정리
2. 실제 runtime 자동 실행 경로 end-to-end smoke 추가
3. release gate / checklist 정리

#### public stable / final
**not yet**

지금 남은 문제는 기능 누락보다는 **출시 정합성** 문제다.

---

## 이미 끝난 것

### A. root package 정합성
- root package/bin이 `axiomnexus`
- root `src/lib.rs`, `src/main.rs` 기준 실행

### B. runtime boundary 수렴
- `WorkspacePort` 제거
- `RuntimePort::execute_turn`
- `ExecuteTurnReq.gate_plan`
- `ExecuteTurnOutcome.observations`

### C. runtime observations 구현
- changed files
- command results
- artifact refs
- notes

### D. 기본 게이트
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `scripts/smoke-runtime.sh`
- `scripts/verify-runtime.sh`

---

## 아직 남은 것

## R-01 — canonical docs 정리
현재 blocker는 “남아 있는 문서”보다 “실제 구현과 안 맞는 문서”다.
특히 CLI / API / release gate 문서가 `scheduler once`, `run once <run_id>`, `verify-release.sh`,
release checklist를 아직 반영하지 못했다.

### 왜 blocker인가
운영자/개발자/릴리스 담당자가 서로 다른 제품을 읽게 된다.

### 종료 조건
- active docs는 오직 AxiomNexus만 설명
- 실제 없는 legacy 파일은 ledger에 이미 정리된 상태로 기록
- docs reader path가 하나만 남음

---

## R-02 — runtime auto path end-to-end smoke
현재 blocker는 canonical auto path를 deterministic하게 검증할 실행 경로가 없다는 점이다.
`queue → wake → run once <run_id> → accepted complete → replay`를 실제로 고정해야 한다.

### 왜 blocker인가
현재 제품의 핵심 자동 경로가 gate 밖에 있다.

### 종료 조건
- `run once <run_id>`를 실제로 1회 실행
- accepted transition 확인
- `TransitionRecord` append 확인
- `WorkSnapshot.rev` 증가 확인
- run state, session, consumption 기록 확인

---

## R-03 — schema / replay / release gates 명문화
현재 docs는 schema drift, replay, adapter conformance, benchmark baseline까지 요구한다.
하지만 release 직전 필수 게이트와 later 목표가 한 묶음으로 남아 있다.

### 왜 blocker인가
release gate가 “오늘 꼭 필요한 것”과 “나중에 할 품질 향상”을 구분하지 못한다.

### 종료 조건
release gate를 2층으로 나눈다.

- ship-now gates
- later hardening gates

---

## R-04 — legacy surface quarantine
legacy surface 자체보다, 실제 없는 legacy 파일을 아직 계획 문서가 계속 참조하는 점이 더 큰 문제다.

### 왜 blocker인가
새 저장소를 열었을 때 active product를 오해한다.

### 종료 조건
- 계획 문서에서 실제 없는 legacy 파일 참조 제거
- root reader path에서 legacy 경로 제거
- root docs는 AxiomNexus만 가리킴

---

## R-05 — release artifacts / operator checklist
현재 verify script는 있다.
하지만 실제 배포 전 무엇을 확인하고 어떤 산출물을 남길지에 대한
canonical release checklist가 없다.

### 종료 조건
- version tag 규칙
- release note template
- smoke log 보관
- export snapshot 보관
- replay summary 보관
- rollback 절차 문서화

---

## 출시 판정

### 출시 가능
아래가 모두 성립하면 **preview release** 가능

- R-01 완료
- R-02 완료
- R-03 중 ship-now gates 완료
- R-05 완료

### 아직 최종 stable 아님
아래가 남아 있으면 stable/final 표시는 미룸

- PostgreSQL adapter
- adapter conformance suite 이중화
- benchmark baseline 저장
- release automation 정착
