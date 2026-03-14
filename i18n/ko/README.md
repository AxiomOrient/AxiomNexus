# AxiomNexus 문서 포털

AxiomNexus는 계약과 증거를 기준으로 work 상태 전이를 통제하는 Rust 기반 IDC control plane입니다.

먼저 읽을 문서:

- [루트 README](../../README.md)
- [문서 인덱스](../../docs/00-index.md)
- [최종 도착지](../../docs/01-FINAL-TARGET.md)
- [청사진](../../docs/02-BLUEPRINT.md)
- [도메인 모델과 불변식](../../docs/03-DOMAIN-AND-INVARIANTS.md)

빠른 시작:

```bash
cargo run -- doctor
cargo run -- contract check
cargo test
scripts/verify-runtime.sh
```
