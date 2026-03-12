# REQ-anx-003 Timeout replay equivalence

## Claim
Timeout requeue behavior must be replayable from authoritative records and produce the same snapshot state as the live store.

## Examples
- replaying queue, claim, and timeout records reconstructs `Todo` after a timeout
- memory and surreal adapters can rebuild the same snapshot from persisted records

## Invariants
- timeout paths emit `TransitionKind::TimeoutRequeue`
- replay succeeds without reading adapter-private mutation state

## Notes
- system transitions are part of the same explanation ledger as human and agent transitions
