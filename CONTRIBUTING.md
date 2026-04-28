# Contribuir a Caimán OS

## Antes de empezar

1. Abre un issue describiendo el cambio
2. Espera feedback antes de implementar features grandes
3. Para bugs: PR directo es bienvenido

## Setup de desarrollo

```bash
git clone https://github.com/your-org/caiman-os
cd caiman-os

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy rustfmt

# Dependencias del sistema
sudo apt-get install clang llvm libelf-dev linux-headers-$(uname -r) bpftool

# UI
cd ui && npm install
```

## Workflow

```bash
# Crear branch
git checkout -b feat/mi-feature

# Desarrollar...

# Verificar antes de hacer PR
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
cd ui && npm run type-check && npm run lint
```

## Convención de commits

```
feat(drs): add sigma threshold configuration
fix(vmm): handle KVM_EXIT_MMIO for unaligned access
docs(install): add iDRAC provisioning guide
test(microseg): add policy compiler unit tests
perf(xdp): use PERCPU maps for deny stats
chore(ci): update kernel versions in matrix
```

## Estructura del monorepo

```
caiman-os/
├── kernel/      módulo kernel C + eBPF
├── vmm/         VMM Rust (KVM directo, sin QEMU)
├── cni/         plugin CNI
├── microseg/    micro-segmentación XDP
├── drs/         DRS scheduler
├── storage/     VSAN + vVols
├── livemig/     live migration
├── gpu/         GPU / MIG
├── bts/         Backup, Templates & Snapshots
├── ui/          dashboard React
└── install/     scripts de instalación
```

## Código de conducta

Sé amable. Todos empezamos desde cero en algún momento.
