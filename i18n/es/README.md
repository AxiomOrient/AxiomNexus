# Portal De Documentacion De AxiomNexus

AxiomNexus es un control plane IDC en Rust que solo confirma cambios de estado cuando el contrato y la evidencia coinciden.

Empieza aqui:

- [README raiz](../../README.md)
- [Indice de docs](../../docs/00-index.md)
- [Diseno del sistema](../../docs/01-system-design.md)
- [Arquitectura objetivo](../../docs/05-target-architecture.md)
- [Plan de implementacion](../../plans/IMPLEMENTATION-PLAN.md)
- [Ledger de tareas](../../plans/TASKS.md)

Inicio rapido:

```bash
cargo run -- doctor
cargo run -- contract check
cargo test
scripts/verify-runtime.sh
```
