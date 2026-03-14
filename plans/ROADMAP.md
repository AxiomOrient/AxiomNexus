# Roadmap

## 목표
설계 변경 없이, 현재 구현을 **출시 가능한 상태**로 마감한다.

---

## Phase A — Canonical Surface Cleanup

### 목적
active reader path를 하나로 만든다.

### 작업
- root README와 `docs/00-index.md`만 canonical 입구로 남김
- 실제 CLI / gate / release 문서를 현재 구현에 맞춤
- 존재하지 않는 script 링크 제거

### 완료 기준
새 사용자가 root README → docs index → spec → plans 순서만 따라도 혼동이 없다.

---

## Phase B — Runtime Execute E2E Gate

### 목적
핵심 자동 경로를 실제 smoke gate에 넣는다.

### 작업
- `scripts/smoke-runtime.sh`를 canonical auto path 기준으로 확장
- `run once <run_id>` direct execute CLI 추가
- `AXIOMNEXUS_COCLAI_SCRIPT_PATH` 기반 scripted runtime smoke 추가
- accepted transition, record append, snapshot rev increment 검증
- session / consumption / replay까지 연결

### 완료 기준
실제 runtime turn 하나가 릴리스 게이트 안에서 끝까지 돈다.

---

## Phase C — Release Gates Split

### 목적
지금 필요한 게이트와 나중 hardening을 분리한다.

### ship-now gates
- fmt
- clippy
- test
- schema drift
- replay
- runtime e2e smoke

### later hardening gates
- postgres adapter conformance
- benchmark baseline
- extended observability audit

### 완료 기준
릴리스 담당자가 “무엇이 출시 필수인가”를 즉시 알 수 있다.

---

## Phase D — Release Pack

### 목적
배포 절차와 증거를 고정한다.

### 작업
- `RELEASE-CHECKLIST.md`를 repo canonical checklist로 반영
- versioning 규칙
- release note template
- smoke output 보관 경로
- export snapshot 보관
- replay summary 보관
- rollback 절차 명시

### 완료 기준
배포가 사람 기억이 아니라 문서와 스크립트로 재현된다.

---

## Phase E — Post-Release Hardening

### 목적
preview 이후 stable/final로 가기 위한 후속 작업

### 작업
- PostgreSQL adapter
- adapter conformance suite 2-store 통과
- benchmark baseline 저장
- tracing / structured diagnostics 강화

### 완료 기준
Surreal-first 구현을 유지하면서도 stable-grade portability를 확보한다.
