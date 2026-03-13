# AxiomNexus 文档入口

AxiomNexus 是一个基于 Rust 的 IDC control plane，只有在合同和证据同时满足时才会提交 work 状态变更。

建议先读这些文档：

- [根 README](../../README.md)
- [文档索引](../../docs/00-index.md)
- [系统设计](../../docs/01-system-design.md)
- [目标架构](../../docs/05-target-architecture.md)
- [实现计划](../../plans/IMPLEMENTATION-PLAN.md)
- [任务台账](../../plans/TASKS.md)

快速开始：

```bash
cargo run -- doctor
cargo run -- contract check
cargo test
scripts/verify-runtime.sh
```
