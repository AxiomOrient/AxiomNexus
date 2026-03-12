# REQ-anx-004 Kernel and model dependency boundary

## Claim
`src/kernel` and `src/model` must stay free of runtime, network, filesystem, and process orchestration dependencies.

## Examples
- kernel files do not import `tokio`, `reqwest`, or `std::process::Command`
- model files remain data-only and serialization-focused

## Invariants
- pure decision and replay logic stays under `src/kernel`
- data contracts and ids stay under `src/model`

## Notes
- boundary checks should fail closed when forbidden tokens appear in kernel or model files
