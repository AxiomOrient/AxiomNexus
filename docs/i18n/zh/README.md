# AxiomNexus 文档入口

AxiomNexus 是一个基于 Rust 的 IDC control plane，只有在合同和证据同时满足时才会提交 work 状态变更。

建议先读这些文档：

- [根 README](../../../README.md)：runtime surface 与 governance surface 总览
- [文档索引](../../00-index.md)：active docs 与 archive 边界
- [系统设计](../../01-system-design.md)：核心模型与边界
- [目标架构](../../05-target-architecture.md)：当前 authoritative architecture
- [Triad 治理集成](../../07-triad-governance-integration-plan.md)：治理集成 surface

快速开始：

```bash
cargo run -- doctor
cargo run -- contract check
cargo run -p axiomnexus-governance -- status
```
