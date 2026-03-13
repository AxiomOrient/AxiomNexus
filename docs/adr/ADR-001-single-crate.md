# ADR-001 — Single Crate 유지

## Status
Accepted

## Decision
AxiomNexus는 workspace로 쪼개지 않고 single-crate modular monolith를 유지한다. [R2]

## Rationale
- 핵심 invariant가 강하게 결합돼 있다.
- hot path가 한 컴파일 단위 안에서 보이는 것이 더 중요하다.
- 초기 runtime은 coclai 하나다. [R1][R2]
- package split은 drift 비용을 먼저 늘린다.

## Consequences
- 상위 모듈은 `boot/model/kernel/app/port/adapter`
- visibility와 trait boundary로만 경계 표현
