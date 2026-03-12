> Archived on 2026-03-12. 이 문서는 완료된 governance task ledger record이며 active task surface가 아닙니다. 현재 integration contract는 [../07-triad-governance-integration-plan.md](../07-triad-governance-integration-plan.md) 에 둡니다.

# Triad Governance Tasks

## Status Rule

- `done`: 코드와 검증 증거가 모두 있다.
- `done-with-bounds`: 구현은 완료됐고, side effect를 피하기 위해 더 좁은 검증 seam으로 닫았다.

| Task ID | Title | Status | Evidence |
| --- | --- | --- | --- |
| `ANX-TRI-001` | package+workspace 전환 | `done` | root `Cargo.toml`이 `.`와 `crates/axiomnexus-governance`를 함께 관리한다 |
| `ANX-TRI-002` | governance crate 추가 | `done` | `crates/axiomnexus-governance`가 build/test 된다 |
| `ANX-TRI-003` | triad path dependency 연결 | `done` | governance crate만 `triad-*`에 의존하고 `cargo tree -p axiomnexus`는 `CLEAN`이다 |
| `ANX-TRI-004` | `triad.toml` 추가 | `done` | `axiomnexus`용 governance config가 root `triad.toml`로 존재하고 parse된다 |
| `ANX-TRI-005` | bootstrap 구현 | `done` | `init`가 `spec/claims`, `.triad/*`, embedded schema export를 생성한다 |
| `ANX-TRI-006` | `AxiomNexusProfile` 구현 | `done` | attachments/write roots/verify mapping이 profile test로 고정된다 |
| `ANX-TRI-007` | first claim pack 추가 | `done` | 여섯 개 초기 claim이 유효한 markdown grammar로 존재한다 |
| `ANX-TRI-008` | governance CLI 구현 | `done` | `init`, `next`, `status`, `work`, `verify`, `accept` command path가 구현됐다 |
| `ANX-TRI-009` | smoke and regression verification | `done-with-bounds` | `init/next/status/verify`는 실제 repo smoke 통과, `work/accept`는 side effect를 피하기 위해 internal runtime seam regression으로 검증 |
| `ANX-TRI-010` | docs index 갱신 | `done` | governance 문서가 `docs/00-index.md`에 반영됐다 |

## Explicit Note On Bounds

`ANX-TRI-009`는 의도적으로 두 층으로 닫았다.

- repo smoke: `init`, `next`, `status`, `verify`
- deterministic regression: `work`, `accept`

이유:

- `work`는 live agent 실행을 수반한다.
- `accept`는 live patch apply side effect를 만든다.
- 이번 통합 단계에서는 command dispatch와 repo wiring의 정확성을 우선 검증하고, live mutation smoke는 범위 밖으로 둔다.
