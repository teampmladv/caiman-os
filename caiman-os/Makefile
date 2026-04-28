# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — Root Makefile
#  Born in Cuba. Built for the cloud.
# ═══════════════════════════════════════════════════════════════════════════

CAIMAN_VERSION ?= 0.1.0
ARCH           ?= x86_64
KERNEL_VER     ?= $(shell uname -r)
BUILDROOT_VER  ?= 2024.02

BRT  := \033[38;2;118;255;3m
NC   := \033[0m
TICK := $(BRT)✓$(NC)

.PHONY: all build iso qemu-test clean lint test docs help \
        build-kernel build-ebpf build-rust build-kmod \
        publish push-images release

help: ## Show this help
	@echo -e "\n$(BRT)🐊 Caimán OS v$(CAIMAN_VERSION)$(NC)\n"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN{FS=":.*?## "}; {printf "  \033[38;2;76;175;80m%-22s\033[0m %s\n", $$1, $$2}'
	@echo

# ── Build targets ──────────────────────────────────────────────────────────

all: build ## Build everything (default)

build: build-ebpf build-kmod build-rust ## Build all components

build-ebpf: ## Compile XDP/eBPF programs (requires clang + BPF headers)
	@echo -e "$(BRT)━━ Building eBPF programs$(NC)"
	clang -O2 -g -target bpf \
		-I/usr/include/$(ARCH)-linux-gnu \
		-c kernel/ebpf/xdp_vm_router.c \
		-o kernel/ebpf/xdp_vm_router.o
	clang -O2 -g -target bpf \
		-I/usr/include/$(ARCH)-linux-gnu \
		-c microseg/ebpf/xdp_microseg.c \
		-o microseg/ebpf/xdp_microseg.o
	@echo -e "$(TICK) eBPF programs compiled"

build-kmod: ## Build caiman_net kernel module
	@echo -e "$(BRT)━━ Building kernel module$(NC)"
	$(MAKE) -C /lib/modules/$(KERNEL_VER)/build \
		M=$(PWD)/kernel/caiman_net \
		modules
	@echo -e "$(TICK) caiman_net.ko built"

build-rust: ## Build all Rust crates
	@echo -e "$(BRT)━━ Building Rust workspace$(NC)"
	cargo build --release --workspace
	@echo -e "$(TICK) Rust workspace built"

build-ui: ## Build React dashboard
	@echo -e "$(BRT)━━ Building UI$(NC)"
	cd ui && npm ci && npm run build
	@echo -e "$(TICK) UI built → ui/dist/"

# ── OS Image ───────────────────────────────────────────────────────────────

setup: ## Download and configure Buildroot
	@echo -e "$(BRT)━━ Setting up Buildroot $(BUILDROOT_VER)$(NC)"
	wget -q -c \
		"https://buildroot.org/downloads/buildroot-$(BUILDROOT_VER).tar.gz" \
		-O /tmp/buildroot.tar.gz
	tar -xf /tmp/buildroot.tar.gz -C /opt/
	ln -sf /opt/buildroot-$(BUILDROOT_VER) /opt/buildroot
	cp buildroot/configs/caiman_defconfig /opt/buildroot/configs/
	@echo -e "$(TICK) Buildroot ready at /opt/buildroot"

iso: ## Build bootable ISO
	@echo -e "$(BRT)━━ Building Caimán OS ISO$(NC)"
	$(MAKE) -C /opt/buildroot \
		BR2_EXTERNAL=$(PWD)/buildroot/external \
		caiman_defconfig
	$(MAKE) -C /opt/buildroot
	cp /opt/buildroot/output/images/caiman.iso .
	sha256sum caiman.iso > caiman.iso.sha256
	@echo -e "$(TICK) ISO: caiman.iso"
	@ls -lh caiman.iso

qemu-test: ## Boot ISO in QEMU for local testing (no KVM needed)
	@echo -e "$(BRT)━━ Booting in QEMU$(NC)"
	qemu-system-x86_64 \
		-enable-kvm \
		-m 4G -smp 4 \
		-drive file=caiman.iso,media=cdrom,if=ide \
		-drive file=test-disk.img,if=virtio \
		-net nic,model=virtio -net user,hostfwd=tcp::2222-:22 \
		-serial stdio \
		-display none \
		-no-reboot \
		2>&1 | grep -v "^$"

# ── Development ────────────────────────────────────────────────────────────

dev: ## Start development environment (API + UI hot-reload)
	@echo -e "$(BRT)━━ Starting dev environment$(NC)"
	VITE_MOCK=true cd ui && npm run dev &
	RUST_LOG=debug cargo run --bin caiman-api

lint: ## Run all linters
	cargo clippy --workspace --all-targets -- -D warnings
	cargo fmt --check --all
	cd ui && npm run lint

test: ## Run all tests
	cargo test --workspace
	cd ui && npm test -- --run

audit: ## Security audit
	cargo audit
	cd ui && npm audit

docs: ## Build documentation
	cargo doc --workspace --no-deps --open

# ── Docker / OCI Images ────────────────────────────────────────────────────

REGISTRY ?= ghcr.io/your-org
IMAGES    := caiman-vmm caiman-cni caiman-mcp caiman-drs caiman-bts caiman-api

push-images: ## Build and push all OCI images
	@for img in $(IMAGES); do \
		echo -e "$(BRT)━━ Building $$img$(NC)"; \
		docker buildx build \
			--platform linux/amd64 \
			--tag $(REGISTRY)/$$img:$(CAIMAN_VERSION) \
			--tag $(REGISTRY)/$$img:latest \
			--push \
			-f docker/Dockerfile.$$img .; \
	done

# ── Kubernetes ─────────────────────────────────────────────────────────────

deploy: ## Deploy Caimán stack to current kubectl context
	kubectl apply -f k8s/

undeploy: ## Remove Caimán stack from cluster
	kubectl delete -f k8s/ --ignore-not-found

# ── Release ────────────────────────────────────────────────────────────────

release: lint test build iso ## Full release build
	@echo -e "$(BRT)━━ Release v$(CAIMAN_VERSION)$(NC)"
	@echo -e "$(TICK) Ready to publish"

# ── Cleanup ────────────────────────────────────────────────────────────────

clean: ## Clean build artifacts
	cargo clean
	rm -f caiman.iso caiman.iso.sha256
	rm -f kernel/ebpf/*.o microseg/ebpf/*.o
	$(MAKE) -C /lib/modules/$(KERNEL_VER)/build \
		M=$(PWD)/kernel/caiman_net clean 2>/dev/null || true
	cd ui && rm -rf dist node_modules
