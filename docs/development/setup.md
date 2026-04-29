# Development Setup

## Prerequisites

```bash
# Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy rustfmt

# Node.js 20 (for the UI)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
sudo apt-get install -y nodejs   # Ubuntu/Debian
# or: sudo dnf install -y nodejs  # CentOS

# System deps
sudo apt-get install -y \
    clang llvm libelf-dev linux-headers-$(uname -r) \
    build-essential pkg-config libssl-dev libsqlite3-dev
```

---

## Build from source

```bash
git clone https://github.com/teampmladv/caiman-os
cd caiman-os

# Build all Rust crates
cargo build --release --workspace

# Build a specific crate
cargo build --release -p caiman-vmm
cargo build --release -p caiman-api

# Build eBPF programs
clang -O2 -g -target bpf \
    -I/usr/include/x86_64-linux-gnu \
    -c kernel/ebpf/xdp_vm_router.c \
    -o kernel/ebpf/xdp_vm_router.o

# Build kernel module
make -C /lib/modules/$(uname -r)/build \
    M=$PWD/kernel/caiman_net modules

# Build the UI
cd ui && npm ci && npm run build && cd ..
```

---

## Run tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p caiman-drs

# UI tests
cd ui && npm test -- --run
```

---

## Run locally (dev mode)

```bash
# Terminal 1: API
RUST_LOG=debug cargo run --bin caiman-api

# Terminal 2: UI (hot reload)
cd ui && VITE_API_URL=http://localhost:8765 npm run dev

# Terminal 3: Run a VM manually
sudo cargo run --bin caiman-vmm -- \
    --kernel /boot/vmlinuz \
    --mem-mib 256 \
    --cpus 1 \
    --vm-id 1
```

---

## Workspace structure

```
caiman-os/
├── Cargo.toml          workspace root
├── Makefile            build targets
├── docker-compose.yml  full stack
│
├── vmm/                KVM VMM (no QEMU)
│   └── src/
│       ├── main.rs     entry point, CLI args
│       ├── kvm/        KVM wrappers (vm, vcpu, loader, memory)
│       ├── virtio/     virtio devices (net, blk, queue, tap)
│       ├── device/     serial console (16550A)
│       ├── ebpf/       BPF map helpers
│       └── netlink_ctrl.rs  caiman_net.ko integration
│
├── api/                REST API + WebSocket
│   └── src/
│       ├── main.rs     Axum server, routes
│       ├── vm/         VM state + runner
│       ├── node/       metrics from /proc
│       └── demo/       in-memory demo mode
│
├── drs/                Distributed Resource Scheduler
│   └── src/
│       ├── main.rs
│       ├── monitor.rs  collect node metrics
│       ├── balancer.rs σ-imbalance algorithm
│       ├── scheduler.rs K8s scheduler extender
│       └── affinity.rs VM placement rules
│
├── bts/                Backup, Templates & Snapshots
├── livemig/            Live migration
├── cni/                CNI plugin
├── mcp/                MCP server
├── cli/                Terminal CLI
├── gpu/                GPU/MIG support
│
├── kernel/
│   ├── caiman_net/     C kernel module + XDP
│   └── ebpf/           XDP programs (clang → BPF bytecode)
│
├── ui/                 React dashboard
│   └── src/
│       ├── pages/      Overview, VMs, DRS, Microseg, Storage, GPU
│       ├── components/ KpiCard, NodeCard, VmRow, DrsPanel...
│       ├── framework/  ActionBus, mutations, shortcuts
│       └── store/      Zustand cluster state
│
├── install/            Install scripts, PXE, Ansible
├── monitoring/         Prometheus rules, Grafana dashboards
├── docker/             Dockerfiles for each service
└── docs/               Documentation
```

---

## Linting and formatting

```bash
# Check everything
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check --all
cd ui && npm run lint && npm run type-check

# Fix formatting
cargo fmt --all
cd ui && npm run lint -- --fix
```

---

## Adding a new API endpoint

1. Add handler function in `api/src/main.rs`
2. Register route in the `Router::new()` block
3. If it needs state, add it to the handler signature
4. Add documentation in `docs/api/rest.md`

Example:
```rust
// Handler
async fn my_endpoint(Path(id): Path<String>) -> impl IntoResponse {
    ok(json!({ "id": id })).into_response()
}

// Route
.route("/api/vms/:id/my-action", post(my_endpoint))
```

---

## Release process

```bash
# 1. Bump versions in all Cargo.toml
VERSION=0.8.0
find . -name "Cargo.toml" -not -path "*/target/*" | \
    xargs sed -i "s/^version = \".*\"/version = \"$VERSION\"/"

# 2. Build and publish images
./publish-images.sh $VERSION --push

# 3. Commit + tag
git add -A
git commit -m "chore: bump version to $VERSION"
git tag v$VERSION
git push origin main v$VERSION

# GitHub Actions automatically creates the release with binaries
```
