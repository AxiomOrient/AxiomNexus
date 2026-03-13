# AxiomNexus 문서 포털

AxiomNexus는 계약과 증거를 기준으로 work 상태 전이를 통제하는 Rust 기반 IDC control plane입니다.

먼저 읽을 문서:

- [루트 README](../../README.md)
- [문서 인덱스](../../docs/00-index.md)
- [시스템 설계](../../docs/01-system-design.md)
- [목표 아키텍처](../../docs/05-target-architecture.md)
- [구현 계획](../../plans/IMPLEMENTATION-PLAN.md)
- [태스크 ledger](../../plans/TASKS.md)

빠른 시작:

```bash
cargo run -- doctor
cargo run -- contract check
cargo test
scripts/verify-runtime.sh
```
