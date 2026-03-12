# REQ-anx-002 Transition record completeness

## Claim
Every authoritative `TransitionRecord` must preserve the explanation data needed to reconstruct why a transition happened and what status changed.

## Examples
- accepted records carry `before_status`, `after_status`, and `reasons`
- replay and activity views can explain a timeout requeue from records alone

## Invariants
- `TransitionRecord` stores `reasons`, `before_status`, `after_status`, and evidence references
- adapters project activity and replay state from record data instead of hand-written status maps

## Notes
- explanation completeness matters more than minimizing record size
