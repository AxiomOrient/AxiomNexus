# Implementation Plan

## Scope Contract

- REQUEST: `docs/`를 기준 설계 문서로 삼되, 현재 저장소 상태를 반영한 실행 준비 계획을 `plans/` 아래에 고정한다.
- TARGET_SCOPE: `repo` (`/Users/axient/repository/AxiomNexus`)
- DONE_CONDITION:
  - `plans/IMPLEMENTATION-PLAN.md`와 `plans/TASKS.md`가 존재한다.
  - 남은 workstream이 "이미 반영된 것"과 "아직 남은 것"으로 분리된다.
  - blocker와 open decision이 숨겨지지 않는다.
  - 각 태스크에 재실행 가능한 검증 방법이 붙는다.
- CONSTRAINTS:
  - data model first
  - state transition rules stay in `src/model` and `src/kernel`
  - HTTP handler에 business rule을 추가하지 않는다
  - 메인 runtime crate는 triad crate를 직접 import하지 않는다
  - 기존 in-flight 변경을 되돌리지 않는다
  - invariant가 바뀌면 docs와 schema를 함께 갱신한다

## Expanded Atomic Path

1. `scout-boundaries`
2. `plan-what-it-does`
3. `plan-how-to-build`
4. `plan-task-breakdown`

## Execution Summary

이번 계획은 실행 완료 상태로 유지합니다.

- canonical runtime asset인 `.agents/*`를 복구했다
- `README.md`, docs index, i18n 포털을 runtime-only 구조에 맞췄다
- `TransitionRecord`에 `run_id`, `session_id`를 추가했다
- `doctor`가 canonical asset 상태를 함께 보고하도록 보강했다
- `scripts/verify-runtime.sh`를 통과했다

세부 증거와 task별 결과는 `plans/TASKS.md`가 canonical ledger다.

## Out Of Scope For This Workspace

- triad companion repository 내부 구현 변경
- triad CLI/config parser 자체 수정
- 외부 저장소가 필요한 smoke test 자동화

이 항목들은 AxiomNexus 쪽 경계가 먼저 확정된 뒤에 외부 저장소에서 따라와야 한다.

## Resolved Decisions

- Governance surface residency
  - 현재 저장소의 공식 표면은 runtime-only repo다.
  - triad 관련 분석 문서는 참고 문서로 두고, 제품 문서는 runtime/control-plane 기준으로 재정렬했다.
- Canonical runtime asset location
  - `.agents/*`를 canonical runtime asset 경로로 유지했다.
  - coclai runtime, contract check, tests는 이 경로를 기준으로 다시 통과한다.
- Wake ordering contract
  - 현재 구현의 deduped set semantics를 공식 계약으로 채택했다.
  - 문서 설명을 구현과 같은 의미로 맞췄다.
