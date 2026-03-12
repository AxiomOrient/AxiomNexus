# Portal De Documentacion De AxiomNexus

AxiomNexus es un control plane IDC en Rust que acepta cambios de estado solo cuando el contrato y la evidencia lo permiten.

Empieza aqui:

- [README raiz](../../../README.md): resumen del surface de runtime y governance
- [Indice de docs](../../00-index.md): docs activas y limite del archivo
- [Diseno del sistema](../../01-system-design.md): modelo central y boundaries
- [Arquitectura objetivo](../../05-target-architecture.md): architecture authoritative actual
- [Integracion de governance con Triad](../../07-triad-governance-integration-plan.md): surface de integracion

Quick start:

```bash
cargo run -- doctor
cargo run -- contract check
cargo run -p axiomnexus-governance -- status
```
