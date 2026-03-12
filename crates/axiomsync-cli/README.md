# axiomsync-cli

`axiomsync-cli`는 `axiomsync` 위의 operator and automation command surface입니다. 이 crate는 인자 파싱, command dispatch, script-friendly 출력, release orchestration 진입점을 소유합니다.

## Ownership
- CLI schema and parsing
- command handlers and runtime handoff
- deterministic operator output
- external web companion handoff
- release pack, benchmark, eval, security audit entrypoints

## Runtime Preparation Policy
- `init`는 bootstrap만 수행합니다.
- retrieval-heavy commands는 runtime prepare를 수행합니다.
- read-only and ops commands는 불필요한 전역 rebuild를 피하기 위해 full prepare를 피합니다.

## External Companion Handoff
- `axiomsync web ...`는 외부 viewer binary를 실행합니다.
- Resolution order:
  - `AXIOMSYNC_WEB_VIEWER_BIN`
  - `axiomsync-webd`
- Viewer/server implementation은 별도 web companion project에 있어야 합니다.

## Operator Commands
```bash
cargo run -p axiomsync-cli -- --help
cargo run -p axiomsync-cli -- init
cargo run -p axiomsync-cli -- add ./docs --target axiom://resources/docs
cargo run -p axiomsync-cli -- search "oauth flow"
cargo run -p axiomsync-cli -- release pack --help
```

## Release Notes
- `release pack`는 contract, build quality, reliability, eval, session memory, security audit, benchmark, operability gate를 묶습니다.
- `--security-audit-mode strict`가 release-grade 기본값입니다.
- `G0`는 executable contract integrity gate입니다.
- Retrieval backend policy는 `memory`만 허용합니다.

## Developer Extension Rule
1. CLI schema는 `src/cli/` 아래에서 바꿉니다.
2. handlers는 얇게 유지하고 business logic은 `axiomsync`로 내립니다.
3. 검증:
```bash
cargo clippy -p axiomsync-cli --all-targets -- -D warnings
cargo test -p axiomsync-cli
```
