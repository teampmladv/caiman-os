//! caiman VMM — minimal virtual machine monitor
//!
//! Replaces QEMU entirely. Uses:
//!  - `/dev/kvm` ioctls directly (via `kvm-ioctls`)
//!  - Our `caiman_net_mod` kernel module for networking (via netlink)
//!  - eBPF maps for XDP datapath setup
//!  - Minimal virtio-net / virtio-blk device emulation in-process

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};

mod device;
mod ebpf;
mod kvm;
mod netlink_ctrl;
mod virtio;

use kvm::{memory::GuestMemory, vcpu::Vcpu, vm::Vm};

// ── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "caiman-vmm", about = "Minimal KVM VMM — no QEMU")]
struct Args {
        /// Path to the kernel bzImage
        #[arg(long)]
        kernel: String,

        /// Path to initrd/initramfs (optional)
        #[arg(long)]
        initrd: Option<String>,

        /// Kernel command line
        #[arg(long, default_value = "console=ttyS0 reboot=k panic=1 nomodules")]
        cmdline: String,

        /// Amount of guest RAM in MiB
        #[arg(long, default_value_t = 256)]
        mem_mib: u64,

        /// Number of vCPUs
        #[arg(long, default_value_t = 1)]
        cpus: u8,

        /// Host network interface for XDP uplink (e.g. "eth0")
        #[arg(long, default_value = "eth0")]
        uplink: String,

        /// Guest MAC address (auto-generated if omitted)
        #[arg(long)]
        mac: Option<String>,

        /// Unique VM ID (used as key in caiman_net_mod)
        #[arg(long, default_value_t = 1)]
        vm_id: u32,
}

// ── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
        tracing_subscriber::fmt()
                .with_env_filter(
                        tracing_subscriber::EnvFilter::from_default_env()
                                .add_directive("vmm=info".parse()?),
                )
                .init();

        let args = Args::parse();
        info!("caiman VMM starting: vm_id={} cpus={} mem={}MiB",
              args.vm_id, args.cpus, args.mem_mib);

        run(args).await
}

async fn run(args: Args) -> Result<()> {
        // ── 1. Create KVM VM ───────────────────────────────────────────────
        let mem_size = args.mem_mib * 1024 * 1024;
        let guest_mem = GuestMemory::new(mem_size)
                .context("allocating guest memory")?;

        let vm = Vm::new(&guest_mem).context("creating KVM VM")?;
        info!("KVM VM created (fd={})", vm.fd());

        // ── 2. Load kernel image ───────────────────────────────────────────
        let kernel_load = kvm::loader::load_kernel(
                &guest_mem,
                &args.kernel,
                args.initrd.as_deref(),
                &args.cmdline,
        )
        .context("loading kernel")?;
        info!("Kernel loaded: entry=0x{:x}", kernel_load.kernel_load.offset);

        // ── 3. Create vCPUs ────────────────────────────────────────────────
        let mut vcpus: Vec<Vcpu> = (0..args.cpus)
                .map(|id| {
                        Vcpu::new(&vm, id as u64, &guest_mem, &kernel_load)
                                .with_context(|| format!("creating vCPU {id}"))
                })
                .collect::<Result<_>>()?;
        info!("{} vCPU(s) configured", vcpus.len());

        // ── 4. Register with caiman_net_mod (kernel module) ───────────────────
        let mac = parse_or_generate_mac(args.mac.as_deref(), args.vm_id);
        netlink_ctrl::vm_add(args.vm_id, &mac, &args.uplink)
                .await
                .context("registering VM with caiman_net_mod")?;
        info!("VM registered with caiman_net_mod: mac={}", fmt_mac(&mac));

        // ── 5. Pin BPF maps and attach XDP program ─────────────────────────
        let bpf_pin_path = format!("/sys/fs/bpf/caiman/vm{}", args.vm_id);
        ebpf::setup_vm_maps(args.vm_id, &mac, &bpf_pin_path)
                .context("setting up eBPF maps")?;
        netlink_ctrl::xdp_attach(args.vm_id, &bpf_pin_path)
                .await
                .context("attaching XDP program")?;
        info!("XDP program attached: {}", bpf_pin_path);

        // ── 6. Create virtio-net device ────────────────────────────────────
        let net_dev = virtio::net::VirtioNet::new(
                &vm,
                mac,
                args.vm_id,
        )
        .context("creating virtio-net device")?;

        // ── 7. Run vCPUs ───────────────────────────────────────────────────
        info!("Starting vCPU(s)…");
        let handles: Vec<_> = vcpus
                .iter_mut()
                .map(|vcpu| vcpu.run())
                .collect();

        // Wait for any vCPU to exit
        for h in handles {
                let _ = h.join();
        }

        // ── 8. Cleanup ─────────────────────────────────────────────────────
        warn!("VM exiting — cleaning up");
        netlink_ctrl::xdp_detach(args.vm_id).await.ok();
        netlink_ctrl::vm_del(args.vm_id).await.ok();
        drop(net_dev);

        info!("VMM shutdown complete");
        Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn parse_or_generate_mac(mac_str: Option<&str>, vm_id: u32) -> [u8; 6] {
        if let Some(s) = mac_str {
                let parts: Vec<u8> = s
                        .split(':')
                        .filter_map(|h| u8::from_str_radix(h, 16).ok())
                        .collect();
                if parts.len() == 6 {
                        return parts.try_into().unwrap();
                }
        }
        // Locally administered unicast MAC derived from vm_id
        let id = vm_id.to_le_bytes();
        [0x02, 0xaa, 0xbb, id[0], id[1], id[2]]
}

fn fmt_mac(mac: &[u8; 6]) -> String {
        mac.map(|b| format!("{b:02x}")).join(":")
}
