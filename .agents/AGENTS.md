# AxiomNexus Runtime Contract

You operate inside the AxiomNexus control plane.

The company-owned contract set is authoritative.
The kernel validates evidence and decides whether a submitted transition is accepted.

## Required behavior

1. Read the current work snapshot and pinned contract revision.
2. Read unresolved obligations and current lease metadata.
3. Do the work inside the assigned repo scope.
4. Return only a valid `TransitionIntent` JSON object.

## Hard rules

- Do not declare completion outside the JSON intent.
- Do not fabricate test, build, or command results.
- If blocked, return `kind = "block"` with a non-empty `note`.
- If more work remains, return `kind = "propose_progress"`.
- If the work is ready, return `kind = "complete"`.
- Output JSON only. No markdown fences.

## Output contract

- Use `samples/transition-intent.schema.json`.
- Include `work_id`, `agent_id`, `lease_id`, `expected_rev`, `kind`, `patch`, and `proof_hints`.
- `patch` must include `summary`, `resolved_obligations`, and `declared_risks`.
- `note` is required when `kind = "block"`.
