# Triad Governance Integration Plan

## Summary

`axiomnexus`는 runtime/control-plane code와 개발 거버넌스 code를 분리한다.

- runtime/control-plane: 기존 `axiomnexus` crate
- dev-governance: 새 `crates/axiomnexus-governance`

`triad`는 별도 저장소로 유지하고, `axiomnexus-governance`가 path dependency로 `triad-core`, `triad-config`, `triad-runtime`을 사용한다.

핵심 목표:

- `triad`를 `axiomnexus`의 개발 프로세스 제어면으로 사용
- `axiomnexus` 메인 runtime 코드에 `triad` 도메인 개념을 섞지 않음
- repo-specific 규칙은 `AxiomNexusProfile`로만 주입

## Current Implementation Status

현재 구현은 아래 경계로 고정됐다.

- `Cargo.toml`은 package + workspace 겸용이다.
- `crates/axiomnexus-governance`만 `triad-*` path dependency를 가진다.
- `axiomnexus` 메인 package는 `triad-*`에 직접 의존하지 않는다.
- `triad.toml`, `spec/claims`, `.triad/evidence.ndjson`, `.triad/schemas/*`가 repo-local governance artifact로 존재한다.
- `axiomnexus-governance` CLI는 `init`, `next`, `status`, `work`, `verify`, `accept`를 제공한다.

## Documentation And Prompt Ownership

문서와 prompt asset은 아래 기준으로 나눈다.

- root `README.md`
  - release-facing entrypoint
  - runtime surface와 governance surface를 한 번에 설명한다
- root `AGENTS.md`
  - repo-wide engineering and governance rules
  - `AxiomNexusProfile`이 work context attachment로 사용한다
- `.agents/AGENTS.md`
  - runtime agent prompt policy의 canonical source
  - `axiomnexus` runtime이 직접 읽는다
- `.agents/skills/transition-executor/SKILL.md`
  - runtime executor skill contract
- `docs/00-index.md`
  - active docs와 archive boundary
- `docs/archive/*`
  - 날짜 고정 review, 완료된 plan, 완료된 task ledger

## Integration Shape

```text
axiomnexus/
├─ Cargo.toml              # package + workspace
├─ triad.toml
├─ spec/claims/
├─ .triad/
└─ crates/
   └─ axiomnexus-governance/
```

`axiomnexus-governance` 책임:

- `triad` runtime facade 로드
- `AxiomNexusProfile` 제공
- repo bootstrap
- governance CLI(`init`, `next`, `status`, `work`, `verify`, `accept`)

## Internal Execution Seam

`axiomnexus-governance` 내부에는 `GovernanceRuntime` seam이 있다.

목적은 하나다.

- `work`와 `accept` command dispatch를 실제 agent 실행이나 patch apply side effect 없이 deterministic하게 검증하기

제약:

- 이 seam은 `crates/axiomnexus-governance` 내부 테스트 seam이다.
- `axiomnexus` main runtime/control-plane에는 노출하지 않는다.
- triad loop semantics를 바꾸지 않고 CLI dispatch만 testable하게 만든다.

## AxiomNexusProfile

### Prompt attachments

- `AGENTS.md` (repo-wide governance rules)
- selected claim
- `docs/05-target-architecture.md`
- `samples/transition-intent.schema.json`

### Allowed write roots

- `src/`
- `docs/`
- `samples/`
- `.agents/`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`

### Protected write roots

- `spec/claims/**`

### Verify mapping

- `Unit`
  - selector가 있으면 `cargo test <selector>`
  - 없으면 `cargo test --lib`
- `Contract`
  - `cargo fmt --all --check`
  - canonical asset/schema path scan
  - `cargo run -- contract check`
- `Integration`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
- `Probe`
  - 현재 사용하지 않음

현재 contract 기준:

- `init/next/status/verify`는 실제 repo smoke로 검증한다.
- `work/accept`는 `GovernanceRuntime` seam을 통한 deterministic regression test로 검증한다.
- live agent 실행이나 live patch apply를 smoke gate에 넣지 않는다.

## Release-Facing Entry Surface

출시 기준에서 운영자가 먼저 보는 surface는 아래 셋이다.

- root `README.md`
- `docs/00-index.md`
- `cargo run -p axiomnexus-governance -- <command>`

historical execution records는 archive로 분리한다.

- `docs/archive/08-triad-governance-tasks.md`

## Repository Artifacts

새로 도입하는 governance artifact:

- `triad.toml`
- `spec/claims/*.md`
- `.triad/evidence.ndjson`
- `.triad/patches/`
- `.triad/runs/`
- `.triad/schemas/`

정책:

- `.triad/evidence.ndjson`는 추적 유지
- `.triad/runs/`와 cache 계열은 비추적
- patch draft 추적 정책은 triad 기본 정책을 유지
- root `README.md`는 release entrypoint로 유지하고, `REQ-anx-006` 대응을 위해 automated governance write scope에 포함한다

## First Claim Pack

초기 claim set:

- `REQ-anx-001` canonical asset/schema path consistency
- `REQ-anx-002` `TransitionRecord` completeness
- `REQ-anx-003` timeout replay equivalence
- `REQ-anx-004` kernel/model forbidden dependency boundary
- `REQ-anx-005` authoritative commit CAS
- `REQ-anx-006` docs/runtime public surface sync

## Done When

- `crates/axiomnexus-governance`가 build 된다
- `triad.toml`, `spec/claims`, `.triad/*` bootstrap이 가능하다
- `AxiomNexusProfile`로 `triad` work/verify loop를 `axiomnexus` repo에 적용할 수 있다
- `axiomnexus` 메인 package에는 `triad-*` 직접 dependency가 없다

## Verified State

아래는 현재 구현에서 실제로 검증된 상태다.

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `cargo run -p axiomnexus-governance -- init`
- `cargo run -p axiomnexus-governance -- next`
- `cargo run -p axiomnexus-governance -- status`
- `cargo run -p axiomnexus-governance -- verify REQ-anx-001`
- `cargo tree -p axiomnexus` 결과 `triad-*` 직접 dependency 없음
