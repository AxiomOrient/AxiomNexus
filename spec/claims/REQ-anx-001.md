# REQ-anx-001 Canonical asset and schema paths

## Claim
AxiomNexus must reference one canonical transition intent schema path and one canonical demo contract sample path across runtime code, governance code, docs, and tests.

## Examples
- transition intent schema references point to `samples/transition-intent.schema.json`
- demo contract sample references point to `samples/company-rust-contract.example.json`

## Invariants
- canonical asset and schema paths do not drift across checked source files
- repository-local tests detect path drift without requiring external services

## Notes
- `.agents/AGENTS.md` and `.agents/skills/transition-executor/SKILL.md` remain the canonical prompt assets
