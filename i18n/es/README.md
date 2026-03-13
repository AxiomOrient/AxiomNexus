# Portal De Documentacion De AxiomNexus

AxiomNexus es un control plane IDC en Rust que solo confirma cambios de estado cuando el contrato y la evidencia coinciden.

Empieza aqui:

- [README raiz](../../README.md)
- [Indice de docs](../../docs/00-index.md)
- [Objetivo final](../../docs/01-FINAL-TARGET.md)
- [Plano](../../docs/02-BLUEPRINT.md)
- [Modelo de dominio e invariantes](../../docs/03-DOMAIN-AND-INVARIANTS.md)
- [Plan de implementacion](../../plans/IMPLEMENTATION-PLAN.md)
- [Ledger de tareas](../../plans/TASKS.md)

Inicio rapido:

```bash
cargo run -- doctor
cargo run -- contract check
cargo test
scripts/verify-runtime.sh
```
