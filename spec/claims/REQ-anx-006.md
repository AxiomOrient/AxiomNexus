# REQ-anx-006 Docs and runtime public surface sync

## Claim
The documented public surface for AxiomNexus must stay aligned with the actual runtime entrypoints, commands, and architecture boundaries.

## Examples
- `README.md` and docs point at the same canonical schema path used by the runtime
- command documentation matches the live `axiomnexus` CLI surface

## Invariants
- docs do not describe removed query layers or obsolete runtime ports
- public-surface tests cover module layout, canonical assets, and boot command shape

## Notes
- docs drift is a contract bug when it changes the apparent operating surface
