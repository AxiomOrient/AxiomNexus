---
name: nexus-skill-adapter
description: Use when working inside AxiomNexus with generic upstream skills and you need the thinnest possible local layer to shape inputs and normalize outputs into evidence that `transition-executor` can turn into a valid `TransitionIntent`.
---

# Nexus / Skill Adapter

Use this skill as a thin local adapter, not as a replacement for upstream skills.

## Purpose

Choose the right generic upstream skill, pass the minimum Nexus-specific context into it, then normalize the result into evidence that can honestly support `propose_progress`, `complete`, or `block`.

## Use Upstream Skills Directly

Prefer the existing generic skills for the real work:

- `build-write-code`
- `check-final-verify`
- `test-run-user-scenarios`
- `workflow-build-execute-plan`
- `control-build-until-done`

This local skill exists only to adapt them to Nexus rules.

## Load First

- `.agents/AGENTS.md`
- `.agents/skills/transition-executor/SKILL.md`
- `README.md`
- `samples/transition-intent.schema.json`
- `samples/execute-turn-output.schema.json`

## When To Use Which Upstream Skill

- Use `build-write-code` for one bounded code change with explicit verification.
- Use `check-final-verify` when the work is done and the remaining question is whether the contract and evidence really support a pass.
- Use `test-run-user-scenarios` when you need realistic operator, runtime, or agent scenarios to produce trustable evidence.
- Use `workflow-build-execute-plan` when a task ledger exists and the work should follow it.
- Use `control-build-until-done` when no task ledger exists and the done contract is already clear.

## Nexus Input Additions

Whichever upstream skill you choose, carry these Nexus facts alongside it:

- pinned contract summary
- current lease id
- expected revision
- unresolved obligations
- allowed repo scope

Do not expand the upstream skill contract more than needed. Pass only the facts that change the honesty of the result.

## Nexus Output Normalization

After the upstream skill finishes, normalize the result into this evidence bundle:

- `changed_files`
- `command_results`
- `artifact_refs`
- `notes`
- `resolved_obligations`
- `declared_risks`
- `verification_gaps`
- `proposed_intent_kind`

If the upstream output is missing one of these, do not invent it. Mark the gap.

## Intent Selection Rules

- `complete`: only when the contract-facing work is satisfied and evidence exists for that claim.
- `propose_progress`: use when meaningful work happened but completion would overclaim.
- `block`: use when required evidence, required surface, or required state is missing and continuing would be dishonest.

## Hard Rules

- Agent self-report never outranks observed evidence.
- Keep runtime-origin intents separate from operator commands.
- `scheduler once` is the canonical operator path.
- `run once <run_id>` is the deterministic diagnostic path.
- If the chosen upstream skill result cannot honestly support `complete`, downgrade to `propose_progress` or `block`.
- Do not fork or rewrite upstream skill contracts locally unless the generic contract truly cannot express the Nexus need.

## Output

Return a short adapter note with:

- chosen upstream skill
- nexus-specific inputs added
- evidence bundle produced
- honest intent kind
- remaining gap, if any

