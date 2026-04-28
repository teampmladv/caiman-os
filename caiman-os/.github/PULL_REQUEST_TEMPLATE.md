## Descripción

<!-- Qué hace este PR y por qué es necesario -->

## Tipo de cambio

- [ ] 🐛 Bug fix
- [ ] ✨ Nueva funcionalidad
- [ ] 💥 Breaking change
- [ ] 📚 Documentación
- [ ] 🔧 Refactoring / performance
- [ ] 🔒 Seguridad
- [ ] 🧪 Tests

## Subsistemas afectados

- [ ] `kernel/caiman_net` — módulo kernel + XDP
- [ ] `vmm` — VMM Rust (KVM directo, sin QEMU)
- [ ] `cni` — plugin CNI
- [ ] `microseg` — micro-segmentación XDP
- [ ] `drs` — Distributed Resource Scheduler
- [ ] `storage` — VSAN / vVols
- [ ] `livemig` — live migration
- [ ] `gpu` — GPU / MIG
- [ ] `bts` — Backup, Templates & Snapshots
- [ ] `ui` — Dashboard React
- [ ] `install` — proceso de instalación
- [ ] `docs` — documentación

## Checklist

- [ ] Tests añadidos / actualizados
- [ ] `cargo clippy` sin warnings
- [ ] `cargo fmt` aplicado
- [ ] Documentación actualizada (si aplica)
- [ ] CHANGELOG actualizado (si es feature o fix)
- [ ] No hay secretos ni credenciales en el código
- [ ] El módulo kernel compila en kernel 6.6+
- [ ] Los programas XDP validan con `bpftool prog load`

## Cómo testear

<!-- Pasos concretos para verificar el cambio -->

```bash
# Ejemplo:
cargo test -p caiman-drs
```

## Screenshots / logs (si aplica)
