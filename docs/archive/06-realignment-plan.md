> Archived on 2026-03-12. 이 문서는 완료된 realignment plan record이며 active execution plan이 아닙니다. 현재 진입 문서는 [../00-index.md](../00-index.md) 입니다.

# 재정렬 계획

이 문서는 현재 worktree를 초기 설계 의도와 다시 맞추기 위해 사용했던 실행 계획의 **완료 기록**입니다.

기준 시점: 2026-03-12

## 현재 상태

재정렬 계획 `P0`부터 `P5`까지 모두 완료했습니다.

완료 조건 검증:

1. `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test` 통과
2. `commit_decision` rev/lease CAS 저장 시점 검증
3. scheduler/reaper 경로의 `TransitionRecord` 보존
4. `TransitionRecord`만으로 이유와 before/after status 설명 가능
5. `app`, `port`, `adapter` 경계가 현재 책임과 일치

## phase status

| Phase | 상태 | 결과 |
| --- | --- | --- |
| `P0` 빌드 계약 복구 | done | canonical asset path 복구와 sample dependency 복구 완료 |
| `P1` authoritative commit 닫기 | done | rev/lease CAS, claim authoritative commit 완료 |
| `P2` explanation source 완성 | done | `reasons`, `before_status`, `after_status`, evidence 보존 완료 |
| `P3` IDC 밖 상태 변경 제거 | done | timeout/reaper를 system transition으로 편입, replay 일치 검증 완료 |
| `P4` 경계 단순화 | done | workspace I/O 이동, authoritative fact load, `app::qry` 제거, role trait 분리, runtime/unused port 단순화 완료 |
| `P5` 문서와 검증 동기화 | done | docs refresh, canonical path 고정, invariant tests 보강 완료 |

## task ledger

| Task ID | 상태 |
| --- | --- |
| `R0-1` | done |
| `R0-2` | done |
| `R0-3` | done |
| `R1-1` | done |
| `R1-2` | done |
| `R1-3` | done |
| `R2-1` | done |
| `R2-2` | done |
| `R2-3` | done |
| `R3-1` | done |
| `R3-2` | done |
| `R3-3` | done |
| `R4-1` | done |
| `R4-2` | done |
| `R4-3` | done |
| `R4-4` | done |
| `R4-5` | done |
| `R4-6` | done |
| `R5-1` | done |
| `R5-2` | done |
| `R5-3` | done |

## 남긴 guard

- canonical asset/schema path consistency test
- replay completeness guards (`before_status`, `after_status`)
- stale rev / stale lease integration tests
- timeout replay integration tests
- reason persistence tests

## 다음 관심사

재정렬 계획 기준 blocker는 남지 않았습니다.

다음 작업은 구조 복구가 아니라 선택입니다.

- memory/surreal store 반복 축소
- HTTP surface 축소
- release/publication 준비
