# ADR-003 — WorkspacePort 제거

## Status
Accepted

## Decision
`WorkspacePort`는 제거한다.  
workspace 관측과 gate command 실행은 `RuntimePort::execute_turn` 결과로 수렴시킨다. [R2][R4][R5]

## Rationale
- current_dir / changed_files / gate command는 한 turn 안의 local action/observation이다. [R5]
- evidence가 turn 밖으로 찢어지지 않게 해야 한다.
- runtime session / cwd / output parsing이 하나의 boundary에 있어야 한다.

## Consequences
- `src/port/workspace.rs` 제거
- `ExecuteTurnOutcome.observations` 추가
- app은 workspace port 대신 runtime observations를 evidence로 변환
