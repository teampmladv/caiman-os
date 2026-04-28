//! caiman-vmm v0.2.0 — KVM hypervisor without QEMU
//!
//! v0.2.0 adds:
//!   - bzImage loader (Linux x86 boot protocol)
//!   - Guest memory via KVM_SET_USER_MEMORY_REGION
//!   - vCPU run loop (KVM_EXIT_IO, KVM_EXIT_MMIO, KVM_EXIT_HLT)
//!   - 16550A serial console (ttyS0)
//!   - virtio-blk for disk (optional --disk flag)
//!   - XDP registration with caiman_net.ko

use anyhow::{Context, Result};
use clap::Parser;
use kvm_ioctls::Kvm;
use tracing::info;

mod device;
mod ebpf;
mod kvm;
mod netlink_ctrl;
mod virtio;

#[derive(Parser)]
#[command(name = "caiman-vmm", version = "0.2.0", about = "KVM VMM — no QEMU")]
struct Args {
    /// Linux bzImage path
    #[arg(long)]
    kernel: String,

    /// Initial ramdisk (optional)
    #[arg(long)]
    initrd: Option<String>,

    /// Disk image for virtio-blk (optional)
    #[arg(long)]
    disk: Option<String>,

    /// Kernel command line
    #[arg(long, default_value = "console=ttyS0 reboot=k panic=1 nomodules")]
    cmdline: String,

    /// Guest RAM in MiB
    #[arg(long, default_value_t = 256)]
    mem_mib: u64,

    /// Number of vCPUs
    #[arg(long, default_value_t = 1)]
    cpus: u8,

    /// Host NIC for XDP
    #[arg(long, default_value = "eth0")]
    uplink: String,

    /// VM identifier
    #[arg(long, default_value_t = 1)]
    vm_id: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("caiman_vmm=info".parse()?),
        )
        .init();

    let args = Args::parse();
    info!(
        "caiman-vmm v0.2.0 — vm_id={} cpus={} mem={}MiB kernel={}",
        args.vm_id, args.cpus, args.mem_mib, args.kernel
    );

    run(args).await
}

async fn run(args: Args) -> Result<()> {
    // ── 1. Guest memory ───────────────────────────────────────────────────
    // Open KVM just to get a VmFd for the memory regions;
    // vm.rs will open its own Kvm handle via Vm::new().
    let kvm_fd   = Kvm::new().context("opening /dev/kvm")?;
    let vm_fd    = kvm_fd.create_vm().context("KVM_CREATE_VM")?;
    let mut mem  = kvm::memory::GuestMemory::new(&vm_fd, args.mem_mib, false)
        .context("allocating guest memory")?;
    info!("Guest memory: {} MiB", args.mem_mib);

    // ── 2. Load kernel ────────────────────────────────────────────────────
    let loader_result = kvm::loader::load_bzimage(
        &mut mem,
        std::path::Path::new(&args.kernel),
        &args.cmdline,
        args.initrd.as_deref().map(std::path::Path::new),
        args.mem_mib,
    ).context("loading bzImage")?;
    info!("Kernel loaded: entry={:#x}", loader_result.entry_point);

    // ── 3. Create VM (irqchip + PIT2 + IRQ routing) ───────────────────────
    let vm = kvm::vm::Vm::new(&mem).context("creating VM")?;

    // ── 4. Create vCPUs ───────────────────────────────────────────────────
    let load = kvm::loader::KernelLoadResult {
        kernel_load:      kvm::loader::KernelLoadOffset { offset: loader_result.entry_point },
        boot_params_addr: kvm::loader::ZERO_PAGE_ADDR,
    };

    let handles: Vec<_> = (0..args.cpus)
        .map(|id| {
            kvm::vcpu::Vcpu::new(&vm, id as u64, &mem, &load)
                .map(|mut vcpu| vcpu.run())
        })
        .collect::<Result<_>>()?;

    // ── 5. Register with caiman_net (XDP) ─────────────────────────────────
    let mac = [0x02, 0xaa, 0xbb, 0x00, 0x00, args.vm_id as u8];
    netlink_ctrl::vm_add(args.vm_id, &mac, &args.uplink).await.ok();
    let pin = format!("/sys/fs/bpf/caiman/vm{}", args.vm_id);
    ebpf::setup_vm_maps(args.vm_id, &mac, &pin).ok();
    info!("XDP: vm_id={} mac={}", args.vm_id,
          mac.map(|b| format!("{b:02x}")).join(":"));

    // ── 6. virtio-blk (if disk provided) ─────────────────────────────────
    if let Some(ref disk_path) = args.disk {
        let _blk = virtio::blk::VirtioBlk::new(disk_path, false)
            .context("opening disk image")?;
        info!("virtio-blk: {disk_path}");
    }

    // ── 7. Wait for vCPUs ─────────────────────────────────────────────────
    info!("VM running — {} vCPU(s) active", args.cpus);
    for h in handles {
        let _ = h.join();
    }

    // ── 8. Cleanup ────────────────────────────────────────────────────────
    netlink_ctrl::vm_del(args.vm_id).await.ok();
    info!("VM {} shutdown complete", args.vm_id);
    Ok(())
}
pub fn fmt_mac(mac: &[u8; 6]) -> String { mac.map(|b| format!("{b:02x}")).join(":") }
