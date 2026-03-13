# ADR-005 — triad는 외부 companion이다

## Status
Accepted

## Decision
triad는 repo-local workspace나 direct crate dependency가 아니라 외부 verification companion으로만 연동한다. [R1][T1]

## Rationale
- triad는 `Claim` 중심 verification kernel이다. [T1]
- AxiomNexus는 work lifecycle / decision ledger control plane이다.
- 둘을 합치면 product surface와 책임이 다시 넓어진다.

## Consequences
- integration boundary는 CLI / JSON report / file bridge
- triad가 AxiomNexus state를 직접 mutate하지 못함
