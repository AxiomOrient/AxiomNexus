# ADR-002 — IDC만 authoritative write path로 둔다

## Status
Accepted

## Decision
모든 상태 변경은 `Intent -> Decide -> Commit` 경로만 authoritative write path로 인정한다. [R1][R2][R3]

## Rationale
- state authority를 한 곳에 모아야 replay가 가능하다.
- agent는 intent만 제출해야 한다.
- `TransitionRecord` append-only ledger가 중심이어야 한다. [R7]

## Consequences
- runtime direct mutation 금지
- transport handler direct rule 금지
- rejected/conflict도 record append
