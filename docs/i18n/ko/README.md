# AxiomNexus 문서 포털

AxiomNexus는 계약과 증거를 기준으로 work 상태 전이를 통제하는 Rust 기반 IDC control plane입니다.

먼저 읽을 문서:

- [루트 README](../../../README.md): runtime surface와 governance surface 요약
- [문서 인덱스](../../00-index.md): active docs와 archive 경계
- [시스템 설계](../../01-system-design.md): 핵심 도메인 모델과 경계
- [목표 아키텍처](../../05-target-architecture.md): 현재 authoritative architecture
- [Governance 통합 설계](../../07-triad-governance-integration-plan.md): `triad` integration surface

빠른 시작:

```bash
cargo run -- doctor
cargo run -- contract check
cargo run -p axiomnexus-governance -- status
```
