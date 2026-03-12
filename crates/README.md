# Packages

이 저장소의 Rust package는 두 개입니다. runtime library와 operator CLI를 분리하지만, web/mobile companion 프로젝트는 여기 넣지 않습니다.

## Package Map
- [`axiomsync`](./axiomsync/README.md): runtime library, persistence, retrieval, session, release evidence
- [`axiomsync-cli`](./axiomsync-cli/README.md): operator and automation command surface

## Out Of Repository
- web companion project
- mobile FFI companion project
- iOS and Android application shells

## Common Commands
```bash
cargo run -p axiomsync-cli -- --help
process-compose --log-file logs/process-compose.log -f process-compose.yaml up
bash scripts/quality_gates.sh
```

## Reader Path
- Start with [../README.md](../README.md)
- Runtime boundary: [`axiomsync`](./axiomsync/README.md)
- CLI boundary: [`axiomsync-cli`](./axiomsync-cli/README.md)
- Contracts and architecture: [../docs/README.md](../docs/README.md)
