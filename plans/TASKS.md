# Tasks

가정: 이 태스크 표의 실행 범위는 현재 저장소다. triad companion repository 변경은 외부 의존 태스크로만 남긴다.

| ID | Goal | Scope | Verification | Depends On | Status | Evidence |
|---|---|---|---|---|---|---|
| PLAN-001 | governance/runtime 자산의 최종 source of truth를 확정한다 | `README.md`, `docs/00-index.md`, active docs | README와 active docs가 같은 최종 구조를 말하고, 삭제된 경로와 남겨 둘 경로가 모순 없이 정리된다 | none | done | runtime-only surface로 정렬했고 `docs/00-index.md`에 triad 분석 문서를 참고 문서로 재배치 |
| PLAN-002 | canonical runtime asset 경로를 복구하거나 재정의한다 | `.agents/AGENTS.md`, `.agents/skills/transition-executor/SKILL.md`, `src/adapter/coclai/assets.rs`, `src/lib.rs`, `README.md` | `cargo test runtime_assets_load_from_canonical_repo_paths -- --nocapture`; `cargo test run_turn_once_commits_runtime_intent_into_snapshot_and_transition_record -- --nocapture` | PLAN-001 | done | `.agents/*` 복구 후 두 테스트 모두 통과 |
| PLAN-003 | README, docs index, i18n 링크 그래프를 실제 파일 배치와 맞춘다 | `README.md`, `docs/00-index.md`, `i18n/*`, `docs/01-system-design.md`, `docs/05-target-architecture.md` | active 문서 링크가 실제로 존재하고 문서 포털이 현재 구조를 가리킨다 | PLAN-001 | done | README, docs index, i18n 포털을 현재 구조로 갱신했고 `scripts/verify-runtime.sh` 통과 |
| PLAN-004 | `TransitionRecord` correlation contract를 완성한다 | `src/model/transition.rs`, `src/app/cmd/submit_intent.rs`, `src/app/cmd/claim_work.rs`, `src/kernel/reaper.rs`, store adapters/tests | record가 가능한 경우 `run_id`와 `session_id`를 포함하고 저장/재생 경로가 그 필드를 유지한다 | PLAN-001 | done | `cargo test run_turn_once_commits_runtime_intent_into_snapshot_and_transition_record -- --nocapture`; `cargo test commit_decision_updates_snapshot_record_session_and_pending_wake_atomically -- --nocapture`; `cargo test commit_decision_updates_snapshot_record_session_and_pending_wake -- --nocapture` 통과 |
| PLAN-005 | wake obligation semantics를 최종 계약으로 고정한다 | `src/model/wake.rs`, `src/kernel/wake.rs`, 관련 docs/tests | dedup semantics와 문서 설명이 일치한다 | PLAN-001 | done | `docs/05-target-architecture.md`를 deduped set semantics에 맞춰 갱신했고 기존 wake 테스트가 유지 통과 |
| PLAN-006 | doctor/contract-check가 선택된 자산 계약을 관측 가능하게 드러내게 만든다 | `src/boot/wire.rs`, 관련 tests/docs | doctor가 canonical runtime asset 상태를 출력하고 contract check가 동일한 자산 계약을 유지한다 | PLAN-001, PLAN-002 | done | `cargo test doctor_asset_summary_loads_canonical_runtime_assets -- --nocapture` 통과, `cargo run --quiet -- doctor`가 asset path/byte를 출력 |
| PLAN-007 | public surface와 전체 gate를 마감한다 | repo-wide docs, schema refs, scripts, code | `cargo fmt --all --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test`; `scripts/smoke-runtime.sh` | PLAN-002, PLAN-003, PLAN-004, PLAN-005, PLAN-006 | done | `scripts/verify-runtime.sh` 통과 |
| EXT-001 | 확정된 경계를 triad companion repo에 반영한다 | external triad repo | current mission scope 밖에서 별도 추적 | PLAN-001 | out_of_scope | 현재 mission scope는 `/Users/axient/repository/AxiomNexus` 단일 저장소로 고정 |

## Resolved Decisions

- Governance surface residency — runtime-only repo를 공식 표면으로 채택하고 triad 관련 문서는 참고 문서로 낮췄다.
- Canonical runtime asset location — `.agents/*`를 유지했다.
- Wake ordering contract — 현재 구현의 deduped set semantics를 공식 계약으로 채택했다.
