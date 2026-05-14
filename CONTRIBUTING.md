# Contributing to Caiman OS

Welcome. This is the day-one guide for anyone joining the project, whether as a contributor, a paid engineer, or a co-founder.

If you have not yet read [ARCHITECTURE.md](./ARCHITECTURE.md), read it first. This document is the operational manual; ARCHITECTURE.md is the conceptual map.

**Expected onboarding time:** half a day. By the end of today you should have Caiman compiling on your laptop and a test VM booting on the shared dev cluster.

---

## 1. Communication

We use two channels, deliberately:

- **Discord** for synchronous chat, daily questions, quick pairing. Join here: `https://discord.gg/caimanos` (TBD — link to be published once the server is set up)
- **GitHub Discussions** for design conversations that need to be searchable later: <https://github.com/Capablanca-Digital/caiman-os/discussions>

Open issues for actionable bugs and feature requests. Use Discussions for "should we do X?" conversations. Use Discord for "this is broken right now, help."

When in doubt: Discord first, file an issue if it turns out to be a real bug.

---

## 2. Prerequisites

You need a Linux workstation. macOS and Windows are not supported for development because the VMM needs `/dev/kvm`. WSL2 works for everything except running the VMM locally.

Required:

- Linux x86_64 host with KVM enabled in BIOS (Intel VT-x or AMD-V)
- Kernel 5.15 or newer (run `uname -r` to check)
- At least 8 GiB of RAM, 20 GiB of disk
- `/dev/kvm` accessible to your user (add yourself to the `kvm` group on most distros)

Confirm KVM works:

    lsmod | grep kvm
    ls -la /dev/kvm

If `/dev/kvm` does not exist, KVM is not enabled and you cannot run the VMM. You can still contribute to anything else (API, UI, storage scaffolding, docs).

---

## 3. Install dependencies

The exact commands depend on your distro. The repo includes `publish-images.sh` which auto-detects the package manager; you can use it as a reference.

**Fedora / RHEL / AlmaLinux / Rocky:**

    sudo dnf install -y \
        gcc make clang llvm \
        openssl-devel sqlite-devel pkgconfig \
        elfutils-libelf-devel kernel-devel \
        bpftool curl git

**Debian / Ubuntu:**

    sudo apt-get update
    sudo apt-get install -y \
        gcc make clang llvm \
        libssl-dev libsqlite3-dev pkg-config \
        libelf-dev linux-headers-$(uname -r) \
        bpftool curl git

**Arch:**

    sudo pacman -S --needed \
        gcc make clang llvm \
        openssl sqlite pkgconf \
        libelf linux-headers \
        bpf curl git

---

## 4. Install Rust

Use rustup. We pin a stable Rust version per release; today (v0.9 alpha) we are on **Rust 1.95.0** or newer.

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"
    rustup component add clippy rustfmt
    rustc --version

If you already have rustup, run `rustup update stable`.

For static musl builds (production binaries), add the musl target:

    rustup target add x86_64-unknown-linux-musl
    sudo dnf install musl-libc musl-libc-static 2>/dev/null || \
        sudo apt-get install -y musl-tools

You only need the musl target if you build release artifacts. For development, the default `gnu` target is fine.

---

## 5. Install Node.js

The dashboard requires Node.js 20 or newer.

    # Fedora/RHEL/Alma
    curl -fsSL https://rpm.nodesource.com/setup_20.x | sudo bash -
    sudo dnf install -y nodejs

    # Debian/Ubuntu
    curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
    sudo apt-get install -y nodejs

    node --version
    npm --version

---

## 6. Clone and build

    git clone https://github.com/Capablanca-Digital/caiman-os.git
    cd caiman-os

Build everything:

    make build

That runs (in order):

1. `build-ebpf` — compile XDP/eBPF programs with clang
2. `build-kmod` — compile the kernel module against your running kernel
3. `build-rust` — `cargo build --release --workspace`

If you only care about Rust:

    cargo build --release --workspace

If you only care about the UI:

    cd ui && npm install && npm run build

First build downloads ~400 dependencies and takes 5–10 minutes. Subsequent builds are seconds.

Common build failures:

- **`linker error: cannot find -lssl`** → install `openssl-devel` / `libssl-dev`
- **`fatal error: 'linux/bpf.h' not found`** → install kernel headers for your running kernel
- **`error: linking with cc failed`** when building the kernel module → install `kernel-devel` matching `uname -r`
- **`musl-gcc: not found`** → install musl toolchain or drop `--target x86_64-unknown-linux-musl`

---

## 7. Run Caiman locally

For frontend-only work (UI, design):

    cd ui && npm run dev

Open <http://localhost:5173>. The UI talks to a backend at `http://localhost:8765` by default; if you have no local backend, set `VITE_API_URL` to a shared dev cluster (ask in Discord for the URL).

For backend work (API, VMM, storage):

    # Terminal 1: API
    ./target/release/caiman-api

    # Terminal 2: UI (proxies to localhost:8765)
    cd ui && npm run dev

Create your first VM via API:

    TOKEN=$(curl -s -X POST http://localhost:8765/auth/token \
        -H 'Content-Type: application/json' \
        -d '{"username":"admin","password":"admin123"}' | jq -r .token)

    curl -X POST http://localhost:8765/api/vms \
        -H "Authorization: Bearer $TOKEN" \
        -H 'Content-Type: application/json' \
        -d '{"name":"test-01","cpus":1,"memMib":256}'

To actually boot the VM you need `/dev/kvm` available. If you do not have KVM on your workstation, use the shared dev cluster (Hetzner bare-metal); ask in Discord for credentials.

---

## 8. The two-environment workflow

The core team works with two environments:

- **Local workstation** for editing, compiling, running tests, and frontend dev
- **Hetzner bare-metal** (`caiman-bare-01`) for actually booting VMs with real KVM

When you change something in the VMM or anything KVM-related, you need to deploy your binary to the Hetzner box to test it. The typical loop:

    # On your workstation
    cargo build --release -p caiman-vmm
    scp target/release/caiman-vmm root@caiman-bare-01:/usr/local/bin/

    # SSH in and restart whatever needs restarting
    ssh root@caiman-bare-01
    systemctl restart caiman-api    # API spawns VMM children; this is usually enough

For UI changes:

    cd ui && npm run build
    rsync -az ui/dist/ root@caiman-bare-01:/var/www/ui.caimanos.com/

Ask in Discord for SSH access to the shared environment.

---

## 9. Code style

These rules are enforced by review. Read them once.

### 9.1 Plain ASCII in Rust source files

Caiman binaries are statically linked against musl. Non-ASCII characters in Rust source (em-dashes, accented letters in comments, box-drawing characters in ASCII-art) consistently break musl builds in ways that are hard to debug.

- Comments, string literals, identifiers: ASCII only.
- User-facing strings (returned via JSON, logged) may contain Unicode — they go through `&str` and are fine at runtime.
- When in doubt: write plain ASCII. The linter will catch you anyway (`scripts/check-ascii.sh`).

### 9.2 Async everywhere, no global locks

All backend code is async on `tokio`. Avoid:

- Global `Mutex<HashMap<_, _>>` for shared state. Use `Arc<DashMap>` or per-VM actors instead.
- `std::sync::Mutex` in async code. Use `tokio::sync::Mutex` only if absolutely necessary.
- Blocking IO inside async functions. Wrap in `tokio::task::spawn_blocking` if you must.

Prefer message passing (`tokio::sync::mpsc`, `broadcast`) over shared state.

### 9.3 Error handling

- Library code: return `Result<T, MyError>` with a typed error using `thiserror`.
- Binary code (`main.rs`, request handlers): use `anyhow::Result` for ergonomics.
- Never `panic!()` in production code paths. `unwrap()` is acceptable only in test code or in startup sequences where failure should crash.

### 9.4 JSON wire format

- All JSON fields use `camelCase` at the wire boundary.
- Rust structs use `snake_case` internally with `#[serde(rename_all = "camelCase")]`.
- Datetimes are ISO 8601 with `Z` suffix (`2026-05-14T10:30:00Z`).

### 9.5 Logging

- Use `tracing` crate, not `println!` or `eprintln!`.
- `info!` for normal operational messages. `warn!` for things to investigate. `error!` for actionable failures. `debug!` for verbose internal state. `trace!` for byte-level detail.
- Include structured fields: `info!(vm_id = %id, "VM started")` instead of `info!("VM {} started", id)`.

### 9.6 Comments

Comment intent, not mechanism. The code shows what; the comment shows why.

    // BAD: this comment is noise
    // increment counter by one
    counter += 1;

    // GOOD: this comment explains why
    // AMD requires KVM_SET_TSS_ADDRESS before KVM_CREATE_IRQCHIP;
    // skipping this causes silent boot failures on Ryzen.
    set_tss_address(vm_fd, 0xfffbd000)?;

### 9.7 TypeScript / React

- Functional components and hooks only. No class components.
- Strict mode TS, no `any` unless absolutely necessary.
- Tailwind utility classes for styling. No new CSS files without team agreement.
- One component per file. Co-locate tests as `Component.test.tsx`.

---

## 10. Git workflow

We work on `main` directly for small fixes and on feature branches for anything that takes more than a day.

### 10.1 Branches

- `main` is always deployable to the dev cluster.
- Feature branches: `feat/<short-name>`, `fix/<short-name>`, `docs/<short-name>`.
- One topic per branch. If you find yourself making three unrelated changes, that is three branches.

### 10.2 Commit messages

We use Conventional Commits. The format is:

    type(scope): short description

    Optional longer body explaining what and why.

Types we use:

- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation only
- `refactor` — code change that does not change behavior
- `perf` — performance improvement
- `test` — adding or fixing tests
- `chore` — tooling, CI, build system
- `style` — formatting only (cargo fmt, prettier)

Scopes follow the crate name: `vmm`, `api`, `ui`, `cni`, `drs`, `storage`, `bts`, `gpu`, `microseg`, `livemig`, `bridge`, `mcp`, `cli`, `kernel`, `website`, `ci`.

Examples:

    feat(vmm): add virtio-rng device
    fix(api): handle WebSocket upgrade without auth header
    docs(architecture): document Caiman Bridge migration flow
    refactor(storage): split pool management into separate module
    chore(ci): add cargo clippy step

Commit body is optional but encouraged for non-trivial changes. Explain why, not what — the diff shows what.

### 10.3 Pull requests

For anything beyond a typo fix:

1. Push your branch.
2. Open a PR against `main`.
3. Fill in the PR template (it will appear automatically).
4. Wait for CI to pass.
5. Request review from at least one other contributor.
6. Address feedback by adding new commits (we squash on merge).

Once approved, the reviewer or a maintainer will merge. We squash-merge so the resulting commit message is the PR title + description.

### 10.4 What requires extra review

Changes to these paths need approval from a core maintainer, not just any contributor:

- `vmm/` — anything touching KVM ioctls
- `kernel/` — kernel module or eBPF programs
- `storage/` (when replication ships) — anything touching the data path
- `livemig/` — anything touching memory transfer
- `.github/workflows/` — CI changes
- Any `Cargo.toml` adding or upgrading a dependency

The current core maintainer is the project owner. As the team grows, this list grows.

---

## 11. Before you submit a PR

Run these locally. CI will catch them anyway, but it is faster to fix on your machine.

    # Format
    cargo fmt --all
    cd ui && npm run format

    # Lint
    cargo clippy --workspace --all-targets -- -D warnings
    cd ui && npm run lint && npm run type-check

    # Test
    cargo test --workspace
    cd ui && npm test

    # Build (full release)
    cargo build --release --workspace
    cd ui && npm run build

If any of these fail, fix before pushing. Repeated PRs with broken CI are tiresome for reviewers.

If you are touching the kernel module or eBPF:

    make build-ebpf
    make build-kmod

---

## 12. Testing

### 12.1 Current state — honest

The project is in alpha and test coverage is poor. There are no unit tests under `tests/` directories today. This is a known gap and one of the first things v1.0 will address.

Until proper test infrastructure lands, the de facto test is:

- Compile the workspace successfully.
- Run the API and UI locally.
- Boot a test VM on the Hetzner cluster.
- Verify console works, start/stop works, delete works.

We track manual test scripts under `scripts/qa/`. If you find a bug, please add a script there that reproduces it.

### 12.2 What we want (v1.0+)

- Unit tests for pure logic (storage layout, schedulers, parsers, format conversions)
- Integration tests for the API with a mocked VMM
- End-to-end tests that boot a real VM via KVM on a CI runner with `/dev/kvm`
- UI component tests for critical user flows (login, create VM, console open)

If you submit a feature, add tests proportionate to the risk. New API endpoint without tests is acceptable for now; new code in `vmm/` or `storage/` without tests is not.

### 12.3 Running existing tests

    cargo test --workspace
    cd ui && npm test

These run today but find very little — there is not much to find yet.

---

## 13. Release process

Releases are cut by a core maintainer. The process:

1. Decide the version per [SemVer](https://semver.org/). Alpha versions are `0.x.y`; stable will be `1.0.0` and later.
2. Update `CHANGELOG.md` with the user-facing changes.
3. Update version in `Cargo.toml` (workspace) and `ui/package.json`.
4. Tag the release: `git tag -a v0.x.y -m "release v0.x.y"`.
5. Push: `git push origin v0.x.y`.
6. CI builds the release artifacts (Docker images, ISO).
7. Manually edit the GitHub Release notes with the changelog.
8. Announce on Discord and Discussions.

Docker images are published to `ghcr.io/Capablanca-Digital/` via `publish-images.sh`. This script is used by maintainers; contributors do not run it.

---

## 14. Security

If you find a security vulnerability, **do not** open a public GitHub issue. Email `security@caimanos.com` directly. We will respond within 72 hours.

See [SECURITY.md](./SECURITY.md) for the full disclosure policy.

For non-security bugs, open an issue normally.

---

## 15. License and IP

Caiman OS is Apache 2.0. By contributing, you agree that your contributions are licensed under the same terms.

We do not currently require a CLA (Contributor License Agreement), but this may change for paid employees and core team members. Capablanca Digital OÜ (the Estonian company that maintains Caiman) may require employees to sign an IP Assignment Agreement separately from contributing to the public repo.

External contributors (people contributing PRs without an employment relationship) contribute under Apache 2.0 inbound = outbound. Your name appears in commit history and CONTRIBUTORS.md.

---

## 16. Code of conduct

Be kind. We all started from zero at some point.

- Critique the code, not the person. "This approach has a race condition" is feedback. "This is bad code" is not.
- Assume good intent. Most weird code came from a deadline, a misunderstanding, or learning something new.
- Disagree explicitly with reasons. "I think we should use approach B because of memory pressure under load" is useful. "Approach A is wrong" is not.
- Tone matters. Read your message before sending. Public chat archives everything forever.
- Harassment, discrimination, or personal attacks are not tolerated and result in immediate removal from project channels.

If something feels off, talk to a maintainer privately on Discord DM or email `conduct@caimanos.com`.

---

## 17. Onboarding checklist

Use this on your first day:

- [ ] Read [README.md](./README.md) (5 min)
- [ ] Read [ARCHITECTURE.md](./ARCHITECTURE.md) (20 min)
- [ ] Join Discord and introduce yourself in `#introductions`
- [ ] Install prerequisites (section 2 + 3)
- [ ] Install Rust and Node.js (section 4 + 5)
- [ ] Clone the repo, run `make build` successfully
- [ ] Run the UI locally with `cd ui && npm run dev`
- [ ] Get SSH access to the shared dev cluster (ask in Discord `#dev-access`)
- [ ] Boot a test VM on the dev cluster and connect to its console via the UI
- [ ] Pick a "good first issue" from <https://github.com/Capablanca-Digital/caiman-os/issues?q=label:%22good+first+issue%22>
- [ ] Open your first PR (a docs fix is fine)

If anything in this checklist fails, ask in Discord. We expect this list to need updates; please open a PR to fix it.

---

## 18. Where to ask for help

- Build / install / setup not working → Discord `#help`
- Design question, "should we do X?" → GitHub Discussions
- Bug or feature request → GitHub Issues
- Security vulnerability → security@caimanos.com
- Conduct concern → conduct@caimanos.com
- Anything else → Discord `#general`

Welcome to Caiman. We are glad you are here.

