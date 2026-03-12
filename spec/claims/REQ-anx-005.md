# REQ-anx-005 Authoritative commit CAS

## Claim
Authoritative commit paths must reject stale writes by re-checking snapshot revision and active lease state at commit time.

## Examples
- stale `expected_rev` is rejected before a transition record is committed
- stale or mismatched lease ownership is rejected before a claim or completion commit lands

## Invariants
- `commit_decision` revalidates `expected_rev` and live lease facts
- memory and surreal stores enforce the same authoritative commit contract

## Notes
- correctness of the last write matters more than preserving optimistic caller assumptions
