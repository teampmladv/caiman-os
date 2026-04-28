# SPDX-License-Identifier: GPL-2.0-only
# Compile eBPF programs with clang/LLVM

CLANG    ?= clang
LLC      ?= llc
BPFTOOL  ?= bpftool
ARCH     := $(shell uname -m | sed 's/x86_64/x86/' | sed 's/aarch64/arm64/')
INCLUDES := -I/usr/include/$(shell uname -m)-linux-gnu \
            -I/usr/include \
            -I.

BPF_CFLAGS := -O2 -g -Wall -target bpf -D__TARGET_ARCH_$(ARCH) $(INCLUDES)

PROGS := xdp_vm_router.o

all: $(PROGS)

%.o: %.c maps.h
	$(CLANG) $(BPF_CFLAGS) -c $< -o $@

# Generate BTF-enriched skeleton (for libbpf CO-RE in Rust vmm)
skel: xdp_vm_router.o
	$(BPFTOOL) gen skeleton xdp_vm_router.o > xdp_vm_router.skel.h

# Pin maps to /sys/fs/bpf/caiman/
pin-maps: xdp_vm_router.o
	mkdir -p /sys/fs/bpf/caiman
	$(BPFTOOL) map create /sys/fs/bpf/caiman/mac_to_ifindex \
	    type hash key 6 value 4 entries 256 name mac_to_ifindex
	$(BPFTOOL) map create /sys/fs/bpf/caiman/tx_port \
	    type devmap_hash key 4 value 8 entries 256 name tx_port
	$(BPFTOOL) map create /sys/fs/bpf/caiman/vm_rx_stats \
	    type percpu_hash key 4 value 16 entries 256 name vm_rx_stats

clean:
	rm -f *.o *.skel.h
